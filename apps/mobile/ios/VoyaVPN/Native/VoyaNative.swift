import Foundation
import NetworkExtension
import React

/**
 * The React Native module the JS `native-transport.ts` reaches.
 *
 * It owns the one `VoyaApp` — the Rust host from `crates/voya-mobile-ffi` —
 * and supplies the three things only the app process can do: publish events to
 * JS, start and stop the system VPN configuration, and run a probe core
 * in-process for a latency test while disconnected.
 *
 * The surface is an envelope on purpose (ADR 0012): one `invoke` in, one JSON
 * answer out, in exactly the wire shape Tauri uses. Nothing about a command is
 * modelled twice.
 */
@objc(VoyaNative)
final class VoyaNative: RCTEventEmitter {
    /// The one event every channel travels on; the channel is in the payload.
    private static let eventName = "VoyaBackendEvent"

    private var app: VoyaApp?
    /// Set when React Native tears the module down. A command that lands
    /// after that must not start a new host: the bridge that would use it is
    /// gone, and a second host is a second set of SQLite connections.
    private var invalidated = false
    /// `invoke` runs on a task per call and the frontend fires several the
    /// moment it mounts, so without this each of them would see no host yet
    /// and build its own — several SQLite connections racing the same
    /// migrations, which the database reports as "database is locked".
    /// Guards `app` and `invalidated`.
    private let hostLock = NSLock()
    /// Where commands wait for the host; see `host()`.
    private let hostQueue = DispatchQueue(label: "app.voyavpn.mobile.host")

    private var listening = false
    /// A lock of its own, never `hostLock`: the host publishes events while
    /// it is being constructed, from its own threads, with `hostLock` held —
    /// a `publish` that waited on that lock would never return.
    private let listeningLock = NSLock()

    override static func requiresMainQueueSetup() -> Bool { false }

    override func supportedEvents() -> [String] { [Self.eventName] }

    override func startObserving() { setListening(true) }

    override func stopObserving() { setListening(false) }

    private func setListening(_ value: Bool) {
        listeningLock.lock()
        listening = value
        listeningLock.unlock()
    }

    override func invalidate() {
        hostLock.lock()
        invalidated = true
        let running = app
        app = nil
        hostLock.unlock()
        running?.shutdown()
        super.invalidate()
    }

    /**
     * Runs one backend command.
     *
     * Rejections carry the serialized `AppError` as the message, which is what
     * `native-transport.ts` parses back into a typed `kind`. The code is the
     * error's own name only so the RN bridge has something to put there.
     */
    @objc(invoke:argsJson:resolve:reject:)
    func invoke(
        _ command: String,
        argsJson: String,
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        Task {
            do {
                let host = try await self.host()
                resolve(try await host.invoke(command: command, argsJson: argsJson))
            } catch let error as CommandError {
                switch error {
                case let .Rejected(appErrorJson):
                    reject("VoyaCommandRejected", appErrorJson, error)
                }
            } catch let error as StartupError {
                // Startup failures carry the same serialized `AppError` the
                // command path rejects with, so the frontend can branch on the
                // typed kind — a rejected database keeps its code and is the
                // one failure the reset flow below recovers from.
                let appErrorJson: String
                switch error {
                case let .Paths(json): appErrorJson = json
                case let .Database(json): appErrorJson = json
                case let .Runtime(json): appErrorJson = json
                }
                reject("VoyaStartupFailed", appErrorJson, error)
            } catch {
                reject("VoyaHostUnavailable", error.localizedDescription, error)
            }
        }
    }

    /**
     * Moves the application's database aside and forgets the host, so the next
     * command reconnects against a fresh one.
     *
     * Called by the startup-failure screen the JS side renders when the
     * database was rejected; on a phone there is no shell to run a manual
     * recovery command in, so this is the only path back to a working app.
     */
    @objc(resetApplicationData:reject:)
    func resetApplicationDataFromHost(
        _ resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        // On `hostQueue`, like starting the host: a reset that ran beside a
        // start could move the database out from under the host being built,
        // and the shutdown below blocks, which a task must not.
        hostQueue.async {
            do {
                try self.resetWithoutHost()
                resolve(NSNull())
            } catch let error as ResetError {
                switch error {
                case let .Failed(appErrorJson):
                    reject("VoyaResetFailed", appErrorJson, error)
                }
            } catch {
                reject("VoyaResetFailed", error.localizedDescription, error)
            }
        }
    }

    /// The host, started on first use.
    ///
    /// Building it opens the database and runs its migrations, and every
    /// command the frontend fires as it mounts waits for that. They wait on
    /// `hostQueue`, not on a lock taken inside the task: a task blocked on a
    /// lock holds one of Swift concurrency's few threads for the whole wait,
    /// and enough of them at once leaves none to run anything else.
    private func host() async throws -> VoyaApp {
        try await withCheckedThrowingContinuation { continuation in
            hostQueue.async {
                continuation.resume(with: Result { try self.startedHost() })
            }
        }
    }

    /// On `hostQueue`. Closes the host before the database is moved aside:
    /// the usual caller has no host — it failed to start — but a database
    /// found corrupt after a good start still has connections open on the
    /// file, and moving it under them leaves them writing to the old one.
    private func resetWithoutHost() throws {
        hostLock.lock()
        let invalidated = self.invalidated
        let running = app
        app = nil
        hostLock.unlock()
        // A module React Native has torn down no longer owns the database:
        // its successor may be opening it right now.
        if invalidated { throw HostUnavailable.invalidated }
        running?.shutdown()

        try resetApplicationData(dataDir: VoyaContainer.dataDirectory().path)
    }

    /// On `hostQueue`. Started once, so a JS reload does not reopen the database.
    private func startedHost() throws -> VoyaApp {
        hostLock.lock()
        defer { hostLock.unlock() }

        if let app { return app }
        if invalidated { throw HostUnavailable.invalidated }
        let started = try VoyaApp(
            dataDir: VoyaContainer.dataDirectory().path,
            locale: Locale.preferredLanguages.first,
            events: EventBridge { [weak self] channel, payloadJson in
                self?.publish(channel: channel, payloadJson: payloadJson)
            },
            tunnel: SystemTunnelHost(),
            probeCore: LibboxProbeCoreHost()
        )
        app = started

        return started
    }

    private func publish(channel: String, payloadJson: String) {
        // Events arrive on the host's own threads; `sendEvent` is main-queue
        // work, and one dropped while JS is not listening is not an error —
        // the frontend refetches what it needs on mount.
        listeningLock.lock()
        let listening = self.listening
        listeningLock.unlock()
        guard listening else { return }
        DispatchQueue.main.async { [weak self] in
            self?.sendEvent(withName: Self.eventName, body: ["channel": channel, "payloadJson": payloadJson])
        }
    }

    /// Why no host can be had; reaches JS as `VoyaHostUnavailable`.
    private enum HostUnavailable: LocalizedError {
        case invalidated

        var errorDescription: String? { "the native module has been invalidated" }
    }

    /// Kept out of `VoyaNative` so the closure does not retain the module.
    private final class EventBridge: EventListener, @unchecked Sendable {
        private let publish: @Sendable (String, String) -> Void

        init(publish: @escaping @Sendable (String, String) -> Void) {
            self.publish = publish
        }

        func onEvent(channel: String, payloadJson: String) {
            publish(channel, payloadJson)
        }
    }
}

/// Where the app and its tunnel provider both read and write.
enum VoyaContainer {
    /**
     * The `Info.plist` key each bundle declares its App Group in.
     *
     * The same key the shared PacketTunnel sources read, so the app and its
     * extension agree by declaring the same thing rather than by two constants
     * that have to be kept equal. It must match the
     * `com.apple.security.application-groups` entitlement beside it.
     */
    private static let appGroupInfoKey = "VoyaAppGroupIdentifier"

    /**
     * The App Group container, or the app's own Documents directory.
     *
     * The fallback is for a simulator build with no App Group entitlement: the
     * UI and every command still work against a database of its own, and only
     * the tunnel — which the simulator cannot run anyway — is unavailable.
     */
    static func dataDirectory() -> URL {
        let identifier = Bundle.main.object(forInfoDictionaryKey: appGroupInfoKey) as? String
        let shared = identifier.flatMap {
            FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: $0)
        }

        return shared ?? FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
    }
}
