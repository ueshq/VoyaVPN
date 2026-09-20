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
 * tunnel is being started with.
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

    func start(handoffJson: String, includeAllNetworks: Bool) throws {
        let manager = try loadOrCreateManager(includeAllNetworks: includeAllNetworks)
        do {
            try manager.connection.startVPNTunnel(options: [
                // `NSString`, not `Data`: the provider reads both, and a string
                // survives the options dictionary without a base64 round trip.
                "runtimeConfigJson": handoffJson as NSString,
            ])
        } catch {
            throw TunnelError.Failed(message: "the tunnel did not start: \(error.localizedDescription)")
        }

        switch waitForSettled(starting: true, timeout: Self.startTimeout, connection: manager.connection) {
        case .ready:
            return
        case .invalid:
            throw TunnelError.PermissionDenied
        case .disconnected:
            throw TunnelError.Failed(message: "the tunnel stopped immediately after starting")
        case .timedOut:
            throw TunnelError.Failed(message: "the tunnel did not come up within \(Int(Self.startTimeout))s")
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
            throw TunnelError.Failed(message: "the tunnel was still up after \(Int(Self.stopTimeout))s")
        }
    }

    /// One of `NativeTunProviderState`'s camelCase names; anything else reads as an error.
    func status() -> String {
        guard let found = try? currentManager() else { return "error" }
        guard let manager = found else { return "missingComponent" }

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
        let manager = (try? currentManager()).flatMap { $0 } ?? NETunnelProviderManager()
        let proto = (manager.protocolConfiguration as? NETunnelProviderProtocol) ?? NETunnelProviderProtocol()
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

        return manager
    }

    private func currentManager() throws -> NETunnelProviderManager? {
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

    /// Turns one of NetworkExtension's completion-handler APIs into a throwing call.
    private func blocking(_ body: (@escaping (Error?) -> Void) -> Void) throws {
        let semaphore = DispatchSemaphore(value: 0)
        var failure: Error?
        body { error in
            failure = error
            semaphore.signal()
        }
        semaphore.wait()

        if let failure {
            let code = (failure as NSError).code
            // `NEVPNErrorConfigurationReadWriteFailed` is what a declined or
            // revoked authorization looks like from here.
            if code == NEVPNError.configurationReadWriteFailed.rawValue {
                throw TunnelError.PermissionDenied
            }
            throw TunnelError.Failed(message: failure.localizedDescription)
        }
    }

    // MARK: - Waiting

    private enum WaitResult { case ready, disconnected, invalid, timedOut }

    /**
     * The same state machine `crates/voya-platform/native/macos_tunnel_wait.h`
     * runs on macOS, and for the same reasons: a start that never reaches
     * `connecting` is a permission failure rather than a slow one, and a stop
     * can race a start still queued in `nesessionmanager`, so a disconnected
     * session only counts after it has stayed that way.
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
                if observedActive && Date().timeIntervalSince(since) >= 1 { return .disconnected }
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
