import Foundation
import Network
import NetworkExtension
import os.log

#if canImport(Libbox)
    import Libbox
#endif

public final class PacketTunnelProvider: NEPacketTunnelProvider {
    private static let logger = Logger(subsystem: PacketTunnelIdentity.subsystem, category: "PacketTunnelProvider")

    /// Starts and stops run here, one at a time. NetworkExtension may ask for
    /// a stop while a start is still bringing libbox up; taken side by side,
    /// the stop found no service yet and returned, and the start then left one
    /// running that nothing would ever close. In turn, a stop that arrives
    /// mid-start waits for it and tears down what it built. Static because
    /// libbox's setup is per process, and one provider process can serve
    /// several tunnel sessions.
    private static let lifecycleQueue = DispatchQueue(label: "\(PacketTunnelIdentity.subsystem).lifecycle")

    #if canImport(Libbox)
        /// libbox keeps one stderr redirect per process and rejects a second,
        /// while one provider process can serve several startTunnel calls.
        /// Touched only on `lifecycleQueue`.
        private static var stderrRedirected = false
        /// The running service. Touched only on `lifecycleQueue`: libbox
        /// could reach it from threads of its own only through its command
        /// socket, which this provider never opens.
        private var commandServer: LibboxCommandServer?
        private lazy var platformInterface = VoyaPacketTunnelPlatformInterface(provider: self)
    #endif

    public override func startTunnel(
        options: [String: NSObject]?,
        completionHandler: @escaping (Error?) -> Void
    ) {
        Self.lifecycleQueue.async {
            do {
                let runtimeConfig = try PacketTunnelRuntime.loadRuntimeConfig(options: options)
                PacketTunnelDiagnostics.shared.configure(runtimeConfig)
                PacketTunnelDiagnostics.shared.writeStatus(state: "starting", breadcrumb: "startTunnel entered")
                try self.startSingBox(runtimeConfig)
                PacketTunnelDiagnostics.shared.writeStatus(state: "running", breadcrumb: "sing-box service started")
                completionHandler(nil)
            } catch {
                Self.logger.error("startTunnel failed: \(error.localizedDescription, privacy: .public)")
                PacketTunnelDiagnostics.shared.writeStatus(
                    state: "failed",
                    lastError: error.localizedDescription,
                    breadcrumb: "startTunnel failed: \(error.localizedDescription)"
                )
                completionHandler(error)
            }
        }
    }

    public override func stopTunnel(
        with reason: NEProviderStopReason,
        completionHandler: @escaping () -> Void
    ) {
        Self.lifecycleQueue.async {
            #if canImport(Libbox)
                let server = self.commandServer
                self.commandServer = nil
                do {
                    try server?.closeService()
                } catch {
                    PacketTunnelDiagnostics.shared.appendProviderLog(
                        "VoyaVPN stop service: \(error.localizedDescription)"
                    )
                }
                self.platformInterface.reset()
                server?.close()
            #endif
            PacketTunnelDiagnostics.shared.writeStatus(state: "stopped", breadcrumb: "stopTunnel reason \(reason.rawValue)")
            completionHandler()
        }
    }

    private func startSingBox(_ runtimeConfig: PacketTunnelRuntimeConfig) throws {
        try PacketTunnelRuntime.validate(runtimeConfig)
        #if canImport(Libbox)
            let paths = try PacketTunnelRuntime.runtimePaths()
            try FileManager.default.createDirectory(at: paths.workingURL, withIntermediateDirectories: true)
            try FileManager.default.createDirectory(at: paths.tempURL, withIntermediateDirectories: true)

            let options = LibboxSetupOptions()
            options.basePath = paths.baseURL.path
            options.workingPath = paths.workingURL.path
            options.tempPath = paths.tempURL.path
            // libbox's log buffer is kept for a command client, and none ever
            // connects; the app reads the core's log through its Clash API.
            // In an extension iOS ends at roughly 50 MB, lines nobody reads
            // are not worth holding.
            options.logMaxLines = 0
            options.debug = false

            var setupError: NSError?
            LibboxSetup(options, &setupError)
            if let setupError {
                throw PacketTunnelProviderError.libboxSetupFailed(setupError.localizedDescription)
            }

            // A Go panic or fatal error inside libbox prints to stderr and
            // exits: no crash report, no stopTunnel, and the host only gets
            // NEVPNConnectionError "internal error". Keep that output on disk.
            // libbox moves a non-empty previous file to "stderr.log.old"
            // first, so the trace of the run that died survives the restart.
            // Losing it must not block the tunnel, so a failure is only logged.
            if !Self.stderrRedirected {
                var stderrError: NSError?
                LibboxRedirectStderr(paths.stderrURL.path, &stderrError)
                if let stderrError {
                    PacketTunnelDiagnostics.shared.appendProviderLog(
                        "stderr redirect failed: \(stderrError.localizedDescription)"
                    )
                } else {
                    Self.stderrRedirected = true
                }
            }

            #if os(iOS)
                // iOS kills a NetworkExtension provider at roughly 50 MB. This
                // makes libbox collect at 10% heap growth, cap the Go heap at
                // 45 MiB, and turn on its OOM killer. The command server reads
                // the flag when it is created, so the call has to sit between
                // LibboxSetup and LibboxNewCommandServer. macOS has no such
                // ceiling, and there it would only cost GC time.
                LibboxSetMemoryLimit(true)
            #endif

            var commandServerError: NSError?
            let server = LibboxNewCommandServer(platformInterface, platformInterface, &commandServerError)
            if let commandServerError {
                throw PacketTunnelProviderError.libboxCommandServerFailed(commandServerError.localizedDescription)
            }
            guard let server else {
                throw PacketTunnelProviderError.libboxCommandServerFailed("LibboxNewCommandServer returned nil.")
            }

            do {
                // `server.start()` is not called: it opens libbox's command
                // socket in the App Group container, and nothing connects to
                // it — the app reads the core through its Clash API, as the
                // Android service and the probe core already do.
                try server.startOrReloadService(runtimeConfig.singboxConfigJson, options: LibboxOverrideOptions())
            } catch {
                server.close()
                // libbox may already have opened the TUN and started the
                // interface monitor before it failed; neither is its to undo.
                platformInterface.reset()
                throw PacketTunnelProviderError.libboxServiceFailed(error.localizedDescription)
            }

            commandServer = server
            PacketTunnelDiagnostics.shared.appendProviderLog("VoyaVPN PacketTunnel started.")
        #else
            throw PacketTunnelProviderError.singBoxRuntimeUnavailable
        #endif
    }

}

#if canImport(Libbox)
    extension PacketTunnelProvider {
        func setTunnelNetworkSettingsAsync(_ settings: NEPacketTunnelNetworkSettings?) async throws {
            try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
                setTunnelNetworkSettings(settings) { error in
                    if let error {
                        continuation.resume(throwing: error)
                    } else {
                        continuation.resume()
                    }
                }
            }
        }
    }

#endif
