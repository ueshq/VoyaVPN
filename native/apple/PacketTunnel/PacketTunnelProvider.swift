import Foundation
import Network
import NetworkExtension
import os.log

#if canImport(Libbox)
    import Libbox
#endif

public final class PacketTunnelProvider: NEPacketTunnelProvider {
    private static let logger = Logger(subsystem: PacketTunnelIdentity.subsystem, category: "PacketTunnelProvider")

    #if canImport(Libbox)
        /// libbox keeps one stderr redirect per process and rejects a second,
        /// while one provider process can serve several startTunnel calls.
        private static var stderrRedirected = false
        private var commandServer: LibboxCommandServer?
        private lazy var platformInterface = VoyaPacketTunnelPlatformInterface(provider: self)
    #endif

    public override func startTunnel(
        options: [String: NSObject]?,
        completionHandler: @escaping (Error?) -> Void
    ) {
        Task.detached(priority: .userInitiated) {
            do {
                let runtimeConfig = try PacketTunnelRuntime.loadRuntimeConfig(options: options)
                PacketTunnelDiagnostics.shared.configure(runtimeConfig)
                PacketTunnelDiagnostics.shared.writeStatus(state: "starting", breadcrumb: "startTunnel entered")
                try await self.startSingBox(runtimeConfig)
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
        #if canImport(Libbox)
            do {
                try commandServer?.closeService()
            } catch {
                commandServer?.writeMessage(2, message: "VoyaVPN stop service: \(error.localizedDescription)")
            }
            platformInterface.reset()
            commandServer?.close()
            commandServer = nil
        #endif
        PacketTunnelDiagnostics.shared.writeStatus(state: "stopped", breadcrumb: "stopTunnel reason \(reason.rawValue)")
        completionHandler()
    }

    public override func handleAppMessage(
        _ messageData: Data,
        completionHandler: ((Data?) -> Void)?
    ) {
        do {
            let runtimeConfig = try JSONDecoder().decode(PacketTunnelRuntimeConfig.self, from: messageData)
            try PacketTunnelRuntime.validate(runtimeConfig)
            #if canImport(Libbox)
                try commandServer?.startOrReloadService(runtimeConfig.singboxConfigJson, options: LibboxOverrideOptions())
                PacketTunnelDiagnostics.shared.writeStatus(state: "running", breadcrumb: "service reloaded from app message")
                completionHandler?(nil)
            #else
                throw PacketTunnelProviderError.singBoxRuntimeUnavailable
            #endif
        } catch {
            PacketTunnelDiagnostics.shared.writeStatus(
                state: "failed",
                lastError: error.localizedDescription,
                breadcrumb: "app message failed: \(error.localizedDescription)"
            )
            completionHandler?(error.localizedDescription.data(using: .utf8))
        }
    }

    private func startSingBox(_ runtimeConfig: PacketTunnelRuntimeConfig) async throws {
        try PacketTunnelRuntime.validate(runtimeConfig)
        #if canImport(Libbox)
            let paths = try PacketTunnelRuntime.runtimePaths()
            try FileManager.default.createDirectory(at: paths.workingURL, withIntermediateDirectories: true)
            try FileManager.default.createDirectory(at: paths.tempURL, withIntermediateDirectories: true)

            let options = LibboxSetupOptions()
            options.basePath = paths.baseURL.path
            options.workingPath = paths.workingURL.path
            options.tempPath = paths.tempURL.path
            options.logMaxLines = 3000
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
                try server.start()
                try server.startOrReloadService(runtimeConfig.singboxConfigJson, options: LibboxOverrideOptions())
            } catch {
                server.close()
                throw PacketTunnelProviderError.libboxServiceFailed(error.localizedDescription)
            }

            commandServer = server
            server.writeMessage(2, message: "VoyaVPN PacketTunnel started.")
            PacketTunnelDiagnostics.shared.appendProviderLog("VoyaVPN PacketTunnel started.")
        #else
            throw PacketTunnelProviderError.singBoxRuntimeUnavailable
        #endif
    }

}

#if canImport(Libbox)
    extension PacketTunnelProvider {
        func writeLog(_ message: String) {
            commandServer?.writeMessage(2, message: message)
            PacketTunnelDiagnostics.shared.appendProviderLog(message)
        }

        func closeService() throws {
            try commandServer?.closeService()
            platformInterface.reset()
        }

        func reloadService() throws {
            let runtimeConfig = try PacketTunnelRuntime.loadRuntimeConfig()
            try PacketTunnelRuntime.validate(runtimeConfig)
            try commandServer?.startOrReloadService(runtimeConfig.singboxConfigJson, options: LibboxOverrideOptions())
            PacketTunnelDiagnostics.shared.writeStatus(state: "running", breadcrumb: "service reloaded")
        }

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
