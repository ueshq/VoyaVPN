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
    private var listening = false
    private let queue = DispatchQueue(label: "app.voyavpn.mobile.native", qos: .userInitiated)

    override static func requiresMainQueueSetup() -> Bool { false }

    override func supportedEvents() -> [String] { [Self.eventName] }

    override func startObserving() { listening = true }

    override func stopObserving() { listening = false }

    override func invalidate() {
        app?.shutdown()
        app = nil
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
                let host = try self.host()
                resolve(try await host.invoke(command: command, argsJson: argsJson))
            } catch let error as CommandError {
                switch error {
                case let .Rejected(appErrorJson):
                    reject("VoyaCommandRejected", appErrorJson, error)
                }
            } catch {
                reject("VoyaHostUnavailable", error.localizedDescription, error)
            }
        }
    }

    /// Starts the host on first use, so a JS reload does not reopen the database.
    private func host() throws -> VoyaApp {
        if let app { return app }
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
        guard listening else { return }
        DispatchQueue.main.async { [weak self] in
            self?.sendEvent(withName: Self.eventName, body: ["channel": channel, "payloadJson": payloadJson])
        }
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
