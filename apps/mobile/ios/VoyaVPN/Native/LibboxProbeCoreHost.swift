import Foundation
import Network

#if canImport(Libbox)
    import Libbox
#endif

/**
 * A sing-box instance in the app process, for latency tests while disconnected.
 *
 * Deliberately not the tunnel's core. The provider runs in a process of its
 * own and is not up at all while disconnected, and a probe run started through
 * it would measure nodes through a tunnel that does not exist. So the app
 * links Libbox too and runs a TUN-less instance from the generated probe
 * config, whose only inbounds are the SOCKS listeners the run measures over.
 *
 * Per-run files live under `PTest/`, separate from the provider's `PT/`.
 * This host calls the service in-process and never starts libbox's optional
 * command listener. In particular, simulator container paths can exceed the
 * 104-byte Unix socket limit; no control socket is needed for a probe.
 *
 * While *connected* this is never asked. `speedtest::running_core` measures
 * through the provider's Clash API instead, because a probe core's own
 * connections would go through the tunnel.
 */
#if canImport(Libbox)
    final class LibboxProbeCoreHost: ProbeCoreHost, @unchecked Sendable {
        private static let workingDirectoryName = "PTest"

        private let lock = NSLock()
        private struct Instance {
            let server: LibboxCommandServer
            let directory: URL
        }

        private var instances: [String: Instance] = [:]
        private var starting = false

        func start(configJson: String) throws -> String {
            lock.lock()
            guard !starting, instances.isEmpty else {
                lock.unlock()
                throw ProbeCoreError.Failed(detail: "a probe core is already running")
            }
            starting = true
            lock.unlock()
            defer {
                lock.lock()
                starting = false
                lock.unlock()
            }
            let coreId = UUID().uuidString
            let paths = try prepareWorkingDirectory(coreId: coreId)
            var started = false
            defer {
                if !started { try? FileManager.default.removeItem(at: paths.base) }
            }

            // Process-wide, so two probe cores at once would fight over it.
            // Nothing starts two: `SpeedtestManager` runs one core per run and
            // holds a lock across the run.
            let options = LibboxSetupOptions()
            options.basePath = paths.base.path
            options.workingPath = paths.working.path
            options.tempPath = paths.temp.path
            options.logMaxLines = 0
            options.debug = false

            var setupError: NSError?
            LibboxSetup(options, &setupError)
            if let setupError {
                throw ProbeCoreError.Failed(detail: "libbox setup failed: \(setupError.localizedDescription)")
            }

            let platform = ProbePlatformInterface()
            var serverError: NSError?
            let server = LibboxNewCommandServer(platform, platform, &serverError)
            if let serverError {
                throw ProbeCoreError.Failed(
                    detail: "the probe core could not be created: \(serverError.localizedDescription)",
                )
            }
            guard let server else {
                throw ProbeCoreError.Failed(detail: "LibboxNewCommandServer returned nil")
            }

            do {
                // start() only opens the external gRPC command listener.
                // The generated SOCKS probe inbounds belong to the service.
                try server.startOrReloadService(configJson, options: LibboxOverrideOptions())
            } catch {
                server.close()
                throw ProbeCoreError.Failed(
                    detail: "the probe core did not start: \(error.localizedDescription)",
                )
            }

            lock.lock()
            instances[coreId] = Instance(server: server, directory: paths.base)
            lock.unlock()
            started = true

            return coreId
        }

        func stop(coreId: String) throws {
            lock.lock()
            defer { lock.unlock() }
            guard let instance = instances.removeValue(forKey: coreId) else { return }

            // Closing the service first stops the inbounds; closing the server
            // releases the instance behind them.
            try? instance.server.closeService()
            instance.server.close()
            try? FileManager.default.removeItem(at: instance.directory)
        }

        private func workingRoot() -> URL {
            VoyaContainer.dataDirectory().appendingPathComponent(Self.workingDirectoryName, isDirectory: true)
        }

        private func prepareWorkingDirectory(coreId: String) throws -> (base: URL, working: URL, temp: URL) {
            // Isolate each run's cache and temporary files. No command socket
            // is created here; normal and failed starts remove their own files.
            let base = workingRoot().appendingPathComponent(String(coreId.prefix(8)), isDirectory: true)
            let working = base.appendingPathComponent("Working", isDirectory: true)
            let temp = base.appendingPathComponent("Temp", isDirectory: true)
            do {
                for directory in [base, working, temp] {
                    try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                }
            } catch {
                try? FileManager.default.removeItem(at: base)
                throw ProbeCoreError.Failed(
                    detail: "the probe working directory could not be created: \(error.localizedDescription)",
                )
            }

            return (base, working, temp)
        }
    }

    /**
     * The platform libbox asks about, for an instance that has no tunnel.
     *
     * Almost every answer is "there is nothing here": the probe config declares
     * SOCKS inbounds only, so libbox never opens a TUN, never looks up a
     * connection owner and never resolves a package name. The interface monitor is
     * real, because sing-box binds outbound connections to the default interface
     * and a probe that ignored it would measure the wrong path.
     *
     * The tunnel's own implementation — the one that answers all of this for real
     * — is `native/apple/PacketTunnel/PacketTunnelPlatform.swift`.
     */
    private final class ProbePlatformInterface: NSObject, LibboxPlatformInterfaceProtocol,
        LibboxCommandServerHandlerProtocol
    {
        private var defaultPathMonitor: NWPathMonitor?

        func openTun(_: LibboxTunOptionsProtocol?, ret0_: UnsafeMutablePointer<Int32>?) throws {
            throw probeError("A probe core has no tunnel to open.")
        }

        func usePlatformAutoDetectControl() -> Bool { false }

        func autoDetectControl(_: Int32) throws {}

        func findConnectionOwner(
            _: Int32,
            sourceAddress _: String?,
            sourcePort _: Int32,
            destinationAddress _: String?,
            destinationPort _: Int32
        ) throws -> LibboxConnectionOwner {
            throw probeError("Connection owner lookup is not available in a probe core.")
        }

        func useProcFS() -> Bool { false }

        func writeLog(_: String?) {}

        func startDefaultInterfaceMonitor(_ listener: LibboxInterfaceUpdateListenerProtocol?) throws {
            guard let listener else { return }

            let monitor = NWPathMonitor()
            defaultPathMonitor = monitor
            // The first path has to be in before `start` returns: libbox dials as
            // soon as the service is up, and a probe bound to no interface fails.
            let semaphore = DispatchSemaphore(value: 0)
            monitor.pathUpdateHandler = { path in
                self.updateDefaultInterface(listener, path)
                semaphore.signal()
                monitor.pathUpdateHandler = { path in
                    self.updateDefaultInterface(listener, path)
                }
            }
            monitor.start(queue: DispatchQueue.global(qos: .utility))
            semaphore.wait()
        }

        private func updateDefaultInterface(_ listener: LibboxInterfaceUpdateListenerProtocol, _ path: Network.NWPath) {
            guard path.status != .unsatisfied, let defaultInterface = path.availableInterfaces.first else {
                listener.updateDefaultInterface("", interfaceIndex: -1, isExpensive: false, isConstrained: false)
                return
            }
            listener.updateDefaultInterface(
                defaultInterface.name,
                interfaceIndex: Int32(defaultInterface.index),
                isExpensive: path.isExpensive,
                isConstrained: path.isConstrained
            )
        }

        func closeDefaultInterfaceMonitor(_: LibboxInterfaceUpdateListenerProtocol?) throws {
            defaultPathMonitor?.cancel()
            defaultPathMonitor = nil
        }

        func getInterfaces() throws -> LibboxNetworkInterfaceIteratorProtocol {
            throw probeError("Interface enumeration is not available in a probe core.")
        }

        /// False: this runs in the app, not in the extension, which is the whole point.
        func underNetworkExtension() -> Bool { false }

        func includeAllNetworks() -> Bool { false }

        func clearDNSCache() {}

        func readWIFIState() -> LibboxWIFIState? { nil }

        func localDNSTransport() -> (any LibboxLocalDNSTransportProtocol)? { nil }

        func systemCertificates() -> (any LibboxStringIteratorProtocol)? { nil }

        func send(_: LibboxNotification?) throws {}

        // MARK: - LibboxCommandServerHandlerProtocol
        //
        // A probe core has no system proxy to report and nobody to reload it;
        // the command server only needs these to answer.

        func serviceReload() throws {}

        func serviceStop() throws {}

        func getSystemProxyStatus() throws -> LibboxSystemProxyStatus {
            let status = LibboxSystemProxyStatus()
            status.available = false
            status.enabled = false

            return status
        }

        func setSystemProxyEnabled(_: Bool) throws {
            throw probeError("A probe core sets no system proxy.")
        }

        func writeDebugMessage(_: String?) {}

        private func probeError(_ message: String) -> NSError {
            NSError(domain: "VoyaProbePlatformInterface", code: -1, userInfo: [NSLocalizedDescriptionKey: message])
        }
    }

#else
    /**
     * A build without Libbox.
     *
     * The app still runs, and everything but a disconnected latency test still
     * works: `ProbeCoreError.Unsupported` reaches the frontend as a
     * "core unavailable" outcome on the measured nodes rather than as a crash
     * or a silent zero.
     */
    final class LibboxProbeCoreHost: ProbeCoreHost, @unchecked Sendable {
        func start(configJson _: String) throws -> String {
            throw ProbeCoreError.Unsupported
        }

        func stop(coreId _: String) throws {}
    }
#endif
