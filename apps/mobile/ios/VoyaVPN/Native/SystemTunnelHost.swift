import Foundation
import NetworkExtension

/**
 * The system VPN configuration, as the Rust host sees it.
 *
 * iOS runs the core inside a `NEPacketTunnelProvider` in a process of its own,
 * and only the containing app may install and start the configuration. The
 * handshake therefore travels inline in `startVPNTunnel(options:)` rather than
 * through a file: the provider cannot read the app's own container, and the
 * App Group container is for rule sets and status, not for the config the
 * tunnel is being started with. The provider therefore refuses a start that
 * arrives without it — the VPN switch in Settings — with an error that says to
 * connect from the app, instead of leaving node credentials on disk for it.
 *
 * Every call blocks until the session has settled, because `NativeTunController`
 * is a synchronous trait: the Rust side runs it on a blocking thread.
 */
final class SystemTunnelHost: TunnelHost, @unchecked Sendable {
    /// Must match the appex's `CFBundleIdentifier`.
    private static let providerBundleIdentifier = "app.voyavpn.mobile.PacketTunnel"
    private static let configurationName = "VoyaVPN"
    /// Long enough for the first-run authorization prompt to be answered.
    private static let startTimeout: TimeInterval = 60
    private static let stopTimeout: TimeInterval = 20
    private static let pollInterval: TimeInterval = 0.1
    /// How long one NetworkExtension call may take before it counts as hung.
    private static let callTimeout: TimeInterval = 30
    /// How long a stop that never saw the session active waits for it to stay
    /// down. Long enough for a start still queued in `nesessionmanager` to
    /// show itself, and far short of `stopTimeout`: the usual case is a
    /// provider that already exited, with the supervisor's thread — and the
    /// user's next Connect behind it — waiting on this call.
    private static let idleStopQuietPeriod: TimeInterval = 3

    /// The installed configuration, kept between calls. The Rust side asks for
    /// `status()` every few seconds; reading the preferences each time was a
    /// round trip to `nesessionmanager` per poll, and a single one that failed
    /// or timed out reported `error`, which takes a healthy tunnel down.
    private let managerLock = NSLock()
    private var manager: NETunnelProviderManager?
    /// `true` once `manager` holds what the preferences said, `nil` included.
    private var managerLoaded = false
    /// Bumped by every configuration change, so a load that was in flight
    /// when one arrived does not store what it read before it.
    private var managerGeneration = 0
    private var configurationObserver: NSObjectProtocol?

    init() {
        configurationObserver = NotificationCenter.default.addObserver(
            forName: .NEVPNConfigurationChange,
            object: nil,
            queue: nil,
        ) { [weak self] _ in
            self?.configurationChanged()
        }
    }

    deinit {
        if let configurationObserver {
            NotificationCenter.default.removeObserver(configurationObserver)
        }
    }

    func start(handoffJson: String, includeAllNetworks: Bool) throws {
        let manager = try loadOrCreateManager(includeAllNetworks: includeAllNetworks)
        do {
            try manager.connection.startVPNTunnel(options: [
                // `NSString`, not `Data`: the provider reads both, and a string
                // survives the options dictionary without a base64 round trip.
                "runtimeConfigJson": handoffJson as NSString,
            ])
        } catch {
            throw TunnelError.Failed(detail: "the tunnel did not start: \(error.localizedDescription)")
        }

        switch waitForSettled(starting: true, timeout: Self.startTimeout, connection: manager.connection) {
        case .ready:
            return
        case .invalid:
            throw TunnelError.PermissionDenied
        case .disconnected:
            throw TunnelError.Failed(detail: "the tunnel stopped immediately after starting")
        case .timedOut:
            // The session is still trying. Left alone it could come up after
            // this has reported a failure, with nothing in the app that
            // believes a tunnel is running.
            manager.connection.stopVPNTunnel()
            throw TunnelError.Failed(detail: "the tunnel did not come up within \(Int(Self.startTimeout))s")
        }
    }

    func stop() throws {
        // Nothing installed is already stopped; a read that failed is not, and
        // says so rather than being reported as a successful stop.
        guard let manager = try currentManager() else { return }
        let connection = manager.connection
        connection.stopVPNTunnel()

        switch waitForSettled(starting: false, timeout: Self.stopTimeout, connection: connection) {
        case .disconnected, .invalid:
            return
        case .ready, .timedOut:
            throw TunnelError.Failed(detail: "the tunnel was still up after \(Int(Self.stopTimeout))s")
        }
    }

    /// One of `NativeTunProviderState`'s camelCase names; anything else reads as an error.
    func status() -> String {
        // `try?` would flatten the optional and lose the difference between
        // "the preferences could not be read" and "nothing is installed" —
        // which are the two states this has to tell apart.
        let found: NETunnelProviderManager?
        do {
            found = try currentManager()
        } catch {
            // A configuration read before still answers for its connection.
            // Only with nothing to ask is a failed read an error: reporting
            // one tears the tunnel down, and a preferences read that timed
            // out says nothing about the tunnel.
            guard let known = lastKnownManager() else { return "error" }
            found = known
        }
        // No configuration is installed until the first connect saves one and
        // raises the system prompt. That is a tunnel not yet started, not a
        // missing component: an appex absent from the build surfaces as
        // `MissingProvider` when a start is attempted.
        guard let manager = found else { return "stopped" }

        switch manager.connection.status {
        case .connected:
            return "running"
        case .connecting, .reasserting:
            return "starting"
        case .disconnected, .disconnecting:
            return "stopped"
        case .invalid:
            return "permissionRequired"
        @unknown default:
            return "error"
        }
    }

    // MARK: - Configuration

    private func loadOrCreateManager(includeAllNetworks: Bool) throws -> NETunnelProviderManager {
        // "No configuration yet" and "the configurations could not be read"
        // are different answers, as `status()` already treats them: creating
        // one on a failed read would save a second VPN profile beside the one
        // that is really there.
        // A start always reads the preferences afresh: creating a configuration
        // because a remembered answer said there was none is the duplicate
        // this function exists to avoid.
        configurationChanged()
        let stored = try currentManager()
        let manager = stored ?? NETunnelProviderManager()
        let storedProto = manager.protocolConfiguration as? NETunnelProviderProtocol
        // Already what this start would save: a save and the reload it needs
        // are two round trips to the system on every connect, and each save
        // announces a configuration change to everything that listens for one.
        if stored != nil,
           manager.isEnabled,
           manager.localizedDescription == Self.configurationName,
           storedProto?.providerBundleIdentifier == Self.providerBundleIdentifier,
           storedProto?.serverAddress == Self.configurationName,
           storedProto?.includeAllNetworks == includeAllNetworks
        {
            return manager
        }
        let proto = storedProto ?? NETunnelProviderProtocol()
        proto.providerBundleIdentifier = Self.providerBundleIdentifier
        // Required and otherwise unused: the provider is chosen by bundle id.
        proto.serverAddress = Self.configurationName
        proto.includeAllNetworks = includeAllNetworks
        manager.protocolConfiguration = proto
        manager.localizedDescription = Self.configurationName
        manager.isEnabled = true

        // Saving is what raises the authorization prompt on a first run; a
        // declined prompt is the user's answer, not a malfunction.
        try blocking { done in
            manager.saveToPreferences { error in done(error) }
        }
        // A freshly saved configuration is not readable until it is reloaded.
        try blocking { done in
            manager.loadFromPreferences { error in done(error) }
        }
        remember(manager)

        return manager
    }

    private func currentManager() throws -> NETunnelProviderManager? {
        managerLock.lock()
        if managerLoaded {
            defer { managerLock.unlock() }
            return manager
        }
        let generation = managerGeneration
        managerLock.unlock()

        let found = try loadManager()

        managerLock.lock()
        defer { managerLock.unlock() }
        if generation == managerGeneration {
            manager = found
            managerLoaded = true
        }
        return found
    }

    /// The configuration last read, whether or not it is still current.
    private func lastKnownManager() -> NETunnelProviderManager? {
        managerLock.lock()
        defer { managerLock.unlock() }
        return manager
    }

    private func remember(_ saved: NETunnelProviderManager) {
        managerLock.lock()
        defer { managerLock.unlock() }
        manager = saved
        managerLoaded = true
    }

    /// The user removed or edited the configuration in Settings, or this app
    /// saved it. The object is kept — it still answers for its connection if
    /// the next read fails — but the next call reads the preferences again.
    private func configurationChanged() {
        managerLock.lock()
        defer { managerLock.unlock() }
        managerLoaded = false
        managerGeneration += 1
    }

    private func loadManager() throws -> NETunnelProviderManager? {
        var found: NETunnelProviderManager?
        try blocking { done in
            NETunnelProviderManager.loadAllFromPreferences { managers, error in
                found = managers?.first { manager in
                    (manager.protocolConfiguration as? NETunnelProviderProtocol)?
                        .providerBundleIdentifier == Self.providerBundleIdentifier
                }
                done(error)
            }
        }

        return found
    }

    /**
     * Turns one of NetworkExtension's completion-handler APIs into a throwing
     * call.
     *
     * The wait is bounded because the Rust side runs this on a thread it is
     * blocking: a handler that never fires — which is what NetworkExtension
     * does where it is unavailable, such as the simulator — would otherwise
     * take the whole command channel down with it and never give it back.
     */
    private func blocking(_ body: (@escaping (Error?) -> Void) -> Void) throws {
        let semaphore = DispatchSemaphore(value: 0)
        var failure: Error?
        body { error in
            failure = error
            semaphore.signal()
        }
        if semaphore.wait(timeout: .now() + Self.callTimeout) == .timedOut {
            throw TunnelError.Failed(
                detail: "the system did not answer within \(Int(Self.callTimeout))s",
            )
        }

        if let failure {
            let code = (failure as NSError).code
            // `NEVPNErrorConfigurationReadWriteFailed` is what a declined or
            // revoked authorization looks like from here.
            if code == NEVPNError.configurationReadWriteFailed.rawValue {
                throw TunnelError.PermissionDenied
            }
            throw TunnelError.Failed(detail: failure.localizedDescription)
        }
    }

    // MARK: - Waiting

    private enum WaitResult { case ready, disconnected, invalid, timedOut }

    /**
     * The same state machine `crates/voya-platform/native/macos_tunnel_wait.h`
     * runs on macOS, and for the same reasons: a start that never reaches
     * `connecting` is a permission failure rather than a slow one, and a stop
     * can race a start still queued in `nesessionmanager`, so a disconnected
     * session only counts after it has stayed that way — for a second once it
     * was seen active, and for `idleStopQuietPeriod` when it never was.
     */
    private func waitForSettled(starting: Bool, timeout: TimeInterval, connection: NEVPNConnection) -> WaitResult {
        let deadline = Date().addingTimeInterval(timeout)
        var progressed = false
        var observedActive = false
        var disconnectedSince: Date?

        while Date() < deadline {
            let current = connection.status
            if starting {
                if current == .connected { return .ready }
                if current == .invalid { return .invalid }
                if current == .disconnected && progressed { return .disconnected }
                if current == .connecting || current == .reasserting { progressed = true }
            } else if current == .disconnected || current == .invalid {
                let since = disconnectedSince ?? Date()
                disconnectedSince = since
                let quietPeriod = observedActive ? 1 : Self.idleStopQuietPeriod
                if Date().timeIntervalSince(since) >= quietPeriod { return .disconnected }
            } else {
                observedActive = true
                disconnectedSince = nil
            }
            Thread.sleep(forTimeInterval: Self.pollInterval)
        }

        if !starting,
           let since = disconnectedSince,
           Date().timeIntervalSince(since) >= 1,
           connection.status == .disconnected || connection.status == .invalid
        {
            return .disconnected
        }

        return .timedOut
    }
}
