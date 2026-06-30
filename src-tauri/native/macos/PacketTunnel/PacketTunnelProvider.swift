import Foundation
import Network
import NetworkExtension
import os.log

#if canImport(Libbox)
    import Libbox
#endif

private let appGroupIdentifier = "group.app.voyavpn.desktop"
private let runtimeConfigRelativePath = "Library/Application Support/VoyaVPN/packet-tunnel-runtime.json"

@objc(PacketTunnelProvider)
public final class PacketTunnelProvider: NEPacketTunnelProvider {
    private static let logger = Logger(subsystem: "app.voyavpn.desktop.PacketTunnel", category: "PacketTunnelProvider")

    #if canImport(Libbox)
        private var commandServer: LibboxCommandServer?
        private lazy var platformInterface = VoyaPacketTunnelPlatformInterface(provider: self)
    #endif

    public override func startTunnel(
        options: [String: NSObject]?,
        completionHandler: @escaping (Error?) -> Void
    ) {
        Task.detached(priority: .userInitiated) {
            do {
                let runtimeConfig = try Self.loadRuntimeConfig()
                try await self.startSingBox(runtimeConfig)
                completionHandler(nil)
            } catch {
                Self.logger.error("startTunnel failed: \(error.localizedDescription, privacy: .public)")
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
        completionHandler()
    }

    public override func handleAppMessage(
        _ messageData: Data,
        completionHandler: ((Data?) -> Void)?
    ) {
        do {
            let runtimeConfig = try JSONDecoder().decode(PacketTunnelRuntimeConfig.self, from: messageData)
            try Self.validate(runtimeConfig)
            #if canImport(Libbox)
                try commandServer?.startOrReloadService(runtimeConfig.singboxConfigJson, options: LibboxOverrideOptions())
                completionHandler?(nil)
            #else
                throw PacketTunnelProviderError.singBoxRuntimeUnavailable
            #endif
        } catch {
            completionHandler?(error.localizedDescription.data(using: .utf8))
        }
    }

    private func startSingBox(_ runtimeConfig: PacketTunnelRuntimeConfig) async throws {
        try Self.validate(runtimeConfig)
        #if canImport(Libbox)
            let paths = try Self.runtimePaths()
            try FileManager.default.createDirectory(at: paths.workingURL, withIntermediateDirectories: true)
            try FileManager.default.createDirectory(at: paths.tempURL, withIntermediateDirectories: true)

            let options = LibboxSetupOptions()
            options.basePath = paths.baseURL.path
            options.workingPath = paths.workingURL.path
            options.tempPath = paths.tempURL.path
            options.logMaxLines = 3000
            options.debug = false
            options.crashReportSource = "VoyaVPN PacketTunnel"

            var setupError: NSError?
            LibboxSetup(options, &setupError)
            if let setupError {
                throw PacketTunnelProviderError.libboxSetupFailed(setupError.localizedDescription)
            }

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
        #else
            throw PacketTunnelProviderError.singBoxRuntimeUnavailable
        #endif
    }

    private static func validate(_ runtimeConfig: PacketTunnelRuntimeConfig) throws {
        guard runtimeConfig.version == 1 else {
            throw PacketTunnelProviderError.unsupportedRuntimeConfig(runtimeConfig.version)
        }
        guard !runtimeConfig.singboxConfigJson.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw PacketTunnelProviderError.emptyRuntimeConfig
        }
    }

    private static func loadRuntimeConfig() throws -> PacketTunnelRuntimeConfig {
        let data = try Data(contentsOf: runtimeConfigURL())
        return try JSONDecoder().decode(PacketTunnelRuntimeConfig.self, from: data)
    }

    private static func runtimePaths() throws -> PacketTunnelRuntimePaths {
        guard let containerURL = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroupIdentifier) else {
            throw PacketTunnelProviderError.missingAppGroupContainer
        }

        let baseURL = containerURL.appendingPathComponent("Library/Application Support/VoyaVPN/PacketTunnel", isDirectory: true)
        return PacketTunnelRuntimePaths(
            baseURL: baseURL,
            workingURL: baseURL.appendingPathComponent("Working", isDirectory: true),
            tempURL: baseURL.appendingPathComponent("Temp", isDirectory: true)
        )
    }

    private static func runtimeConfigURL() throws -> URL {
        guard let containerURL = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroupIdentifier) else {
            throw PacketTunnelProviderError.missingAppGroupContainer
        }

        return containerURL.appendingPathComponent(runtimeConfigRelativePath)
    }
}

private struct PacketTunnelRuntimePaths {
    let baseURL: URL
    let workingURL: URL
    let tempURL: URL
}

private struct PacketTunnelRuntimeConfig: Codable {
    let version: Int
    let activeProfileId: String?
    let mainConfigPath: String
    let singboxConfigJson: String
}

private enum PacketTunnelProviderError: LocalizedError {
    case missingAppGroupContainer
    case emptyRuntimeConfig
    case unsupportedRuntimeConfig(Int)
    case singBoxRuntimeUnavailable
    case libboxSetupFailed(String)
    case libboxCommandServerFailed(String)
    case libboxServiceFailed(String)

    var errorDescription: String? {
        switch self {
        case .missingAppGroupContainer:
            return "VoyaVPN App Group container is unavailable."
        case .emptyRuntimeConfig:
            return "VoyaVPN PacketTunnel runtime config is empty."
        case .unsupportedRuntimeConfig(let version):
            return "VoyaVPN PacketTunnel runtime config version \(version) is not supported."
        case .singBoxRuntimeUnavailable:
            return "VoyaVPN PacketTunnel requires Libbox.xcframework. Build it with `pnpm native:macos:libbox` or set VOYAVPN_LIBBOX_XCFRAMEWORK."
        case .libboxSetupFailed(let message):
            return "VoyaVPN PacketTunnel failed to set up libbox: \(message)"
        case .libboxCommandServerFailed(let message):
            return "VoyaVPN PacketTunnel failed to create libbox command server: \(message)"
        case .libboxServiceFailed(let message):
            return "VoyaVPN PacketTunnel failed to start sing-box service: \(message)"
        }
    }
}

#if canImport(Libbox)
    private final class VoyaPacketTunnelPlatformInterface: NSObject, LibboxPlatformInterfaceProtocol, LibboxCommandServerHandlerProtocol {
        private weak var provider: PacketTunnelProvider?
        private var networkSettings: NEPacketTunnelNetworkSettings?
        private var defaultPathMonitor: NWPathMonitor?

        init(provider: PacketTunnelProvider) {
            self.provider = provider
        }

        func openTun(_ options: LibboxTunOptionsProtocol?, ret0_: UnsafeMutablePointer<Int32>?) throws {
            try runBlocking {
                try await self.openTunAsync(options, ret0_)
            }
        }

        private func openTunAsync(_ options: LibboxTunOptionsProtocol?, _ ret0_: UnsafeMutablePointer<Int32>?) async throws {
            guard let provider else {
                throw platformError("PacketTunnel provider is unavailable.")
            }
            guard let options else {
                throw platformError("Missing libbox TUN options.")
            }
            guard let ret0_ else {
                throw platformError("Missing libbox TUN return pointer.")
            }

            let settings = try makeNetworkSettings(options)
            networkSettings = settings
            try await provider.setTunnelNetworkSettingsAsync(settings)

            if let fileDescriptor = provider.packetFlow.value(forKeyPath: "socket.fileDescriptor") as? Int32 {
                ret0_.pointee = fileDescriptor
                return
            }

            let fileDescriptor = LibboxGetTunnelFileDescriptor()
            guard fileDescriptor != -1 else {
                throw platformError("PacketTunnel file descriptor is unavailable.")
            }
            ret0_.pointee = fileDescriptor
        }

        private func makeNetworkSettings(_ options: LibboxTunOptionsProtocol) throws -> NEPacketTunnelNetworkSettings {
            let settings = NEPacketTunnelNetworkSettings(tunnelRemoteAddress: "127.0.0.1")
            if options.getAutoRoute() {
                settings.mtu = NSNumber(value: options.getMTU())
                settings.dnsSettings = try makeDNSSettings(options)
                settings.ipv4Settings = makeIPv4Settings(options)
                settings.ipv6Settings = makeIPv6Settings(options)
            }
            if options.isHTTPProxyEnabled() {
                settings.proxySettings = makeProxySettings(options)
            }
            return settings
        }

        private func makeDNSSettings(_ options: LibboxTunOptionsProtocol) throws -> NEDNSSettings? {
            guard options.getDNSMode()?.value != LibboxDNSModeDisabled else {
                return nil
            }

            let serverIterator = try options.getDNSServerAddress()
            var servers: [String] = []
            while serverIterator.hasNext() {
                servers.append(serverIterator.next())
            }
            guard !servers.isEmpty else {
                return nil
            }

            let dnsSettings = NEDNSSettings(servers: servers)
            dnsSettings.matchDomains = [""]
            dnsSettings.matchDomainsNoSearch = true
            return dnsSettings
        }

        private func makeIPv4Settings(_ options: LibboxTunOptionsProtocol) -> NEIPv4Settings {
            var addresses: [String] = []
            var masks: [String] = []
            let addressIterator = options.getInet4Address()!
            while addressIterator.hasNext() {
                let prefix = addressIterator.next()!
                addresses.append(prefix.address())
                masks.append(prefix.mask())
            }

            let settings = NEIPv4Settings(addresses: addresses, subnetMasks: masks)
            var includedRoutes: [NEIPv4Route] = []
            let routeIterator = options.getInet4RouteAddress()!
            while routeIterator.hasNext() {
                let prefix = routeIterator.next()!
                includedRoutes.append(NEIPv4Route(destinationAddress: prefix.address(), subnetMask: prefix.mask()))
            }
            if includedRoutes.isEmpty {
                includedRoutes.append(.default())
            }

            var excludedRoutes: [NEIPv4Route] = []
            let excludeIterator = options.getInet4RouteExcludeAddress()!
            while excludeIterator.hasNext() {
                let prefix = excludeIterator.next()!
                excludedRoutes.append(NEIPv4Route(destinationAddress: prefix.address(), subnetMask: prefix.mask()))
            }

            settings.includedRoutes = includedRoutes
            settings.excludedRoutes = excludedRoutes
            return settings
        }

        private func makeIPv6Settings(_ options: LibboxTunOptionsProtocol) -> NEIPv6Settings {
            var addresses: [String] = []
            var prefixes: [NSNumber] = []
            let addressIterator = options.getInet6Address()!
            while addressIterator.hasNext() {
                let prefix = addressIterator.next()!
                addresses.append(prefix.address())
                prefixes.append(NSNumber(value: prefix.prefix()))
            }

            let settings = NEIPv6Settings(addresses: addresses, networkPrefixLengths: prefixes)
            var includedRoutes: [NEIPv6Route] = []
            let routeIterator = options.getInet6RouteAddress()!
            while routeIterator.hasNext() {
                let prefix = routeIterator.next()!
                includedRoutes.append(NEIPv6Route(destinationAddress: prefix.address(), networkPrefixLength: NSNumber(value: prefix.prefix())))
            }
            if includedRoutes.isEmpty {
                includedRoutes.append(.default())
            }

            var excludedRoutes: [NEIPv6Route] = []
            let excludeIterator = options.getInet6RouteExcludeAddress()!
            while excludeIterator.hasNext() {
                let prefix = excludeIterator.next()!
                excludedRoutes.append(NEIPv6Route(destinationAddress: prefix.address(), networkPrefixLength: NSNumber(value: prefix.prefix())))
            }

            settings.includedRoutes = includedRoutes
            settings.excludedRoutes = excludedRoutes
            return settings
        }

        private func makeProxySettings(_ options: LibboxTunOptionsProtocol) -> NEProxySettings {
            let settings = NEProxySettings()
            let server = NEProxyServer(address: options.getHTTPProxyServer(), port: Int(options.getHTTPProxyServerPort()))
            settings.httpServer = server
            settings.httpsServer = server
            settings.httpEnabled = true
            settings.httpsEnabled = true

            var bypassDomains: [String] = []
            let bypassIterator = options.getHTTPProxyBypassDomain()!
            while bypassIterator.hasNext() {
                bypassDomains.append(bypassIterator.next())
            }
            if !bypassDomains.isEmpty {
                settings.exceptionList = bypassDomains
            }

            var matchDomains: [String] = []
            let matchIterator = options.getHTTPProxyMatchDomain()!
            while matchIterator.hasNext() {
                matchDomains.append(matchIterator.next())
            }
            if !matchDomains.isEmpty {
                settings.matchDomains = matchDomains
            }

            return settings
        }

        func usePlatformAutoDetectControl() -> Bool {
            false
        }

        func autoDetectControl(_: Int32) throws {}

        func findConnectionOwner(
            _ ipProtocol: Int32,
            sourceAddress: String?,
            sourcePort: Int32,
            destinationAddress: String?,
            destinationPort: Int32
        ) throws -> LibboxConnectionOwner {
            throw platformError("Connection owner lookup is not available in VoyaVPN PacketTunnel.")
        }

        func useProcFS() -> Bool {
            false
        }

        func writeLog(_ message: String?) {
            guard let message else {
                return
            }
            provider?.writeLog(message)
        }

        func startDefaultInterfaceMonitor(_ listener: LibboxInterfaceUpdateListenerProtocol?) throws {
            guard let listener else {
                return
            }

            let monitor = NWPathMonitor()
            defaultPathMonitor = monitor
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

        private func updateDefaultInterface(_ listener: LibboxInterfaceUpdateListenerProtocol, _ path: NWPath) {
            guard path.status != .unsatisfied,
                  let defaultInterface = path.availableInterfaces.first
            else {
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
            guard let defaultPathMonitor else {
                throw platformError("Default interface monitor is not started.")
            }

            let path = defaultPathMonitor.currentPath
            guard path.status != .unsatisfied else {
                return VoyaNetworkInterfaceIterator([])
            }

            let interfaces = path.availableInterfaces.map { item in
                let result = LibboxNetworkInterface()
                result.name = item.name
                result.index = Int32(item.index)
                switch item.type {
                case .wifi:
                    result.type = LibboxInterfaceTypeWIFI
                case .cellular:
                    result.type = LibboxInterfaceTypeCellular
                case .wiredEthernet:
                    result.type = LibboxInterfaceTypeEthernet
                default:
                    result.type = LibboxInterfaceTypeOther
                }
                return result
            }
            return VoyaNetworkInterfaceIterator(interfaces)
        }

        func underNetworkExtension() -> Bool {
            true
        }

        func includeAllNetworks() -> Bool {
            false
        }

        func clearDNSCache() {
            guard let provider, let networkSettings else {
                return
            }

            runBlocking {
                provider.reasserting = true
                defer {
                    provider.reasserting = false
                }
                try? await provider.setTunnelNetworkSettingsAsync(nil)
                try? await provider.setTunnelNetworkSettingsAsync(networkSettings)
            }
        }

        func readWIFIState() -> LibboxWIFIState? {
            nil
        }

        func readWIFISSID() -> String? {
            nil
        }

        func connectSSHAgent(_ ret0_: UnsafeMutablePointer<Int32>?) throws {
            throw platformError("SSH agent forwarding is not supported.")
        }

        func serviceStop() throws {
            try provider?.closeService()
        }

        func serviceReload() throws {
            try provider?.reloadService()
        }

        func getSystemProxyStatus() throws -> LibboxSystemProxyStatus {
            let status = LibboxSystemProxyStatus()
            guard let proxySettings = networkSettings?.proxySettings,
                  proxySettings.httpServer != nil
            else {
                return status
            }

            status.available = true
            status.enabled = proxySettings.httpEnabled
            return status
        }

        func setSystemProxyEnabled(_ isEnabled: Bool) throws {
            guard let provider, let networkSettings, let proxySettings = networkSettings.proxySettings else {
                return
            }
            guard proxySettings.httpServer != nil, proxySettings.httpEnabled != isEnabled else {
                return
            }

            proxySettings.httpEnabled = isEnabled
            proxySettings.httpsEnabled = isEnabled
            networkSettings.proxySettings = proxySettings
            try runBlocking {
                try await provider.setTunnelNetworkSettingsAsync(networkSettings)
            }
        }

        func triggerNativeCrash() throws {
            DispatchQueue.global().asyncAfter(deadline: .now() + .milliseconds(200)) {
                fatalError("VoyaVPN debug native crash")
            }
        }

        func writeDebugMessage(_ message: String?) {
            guard let message else {
                return
            }
            os_log("%{public}@", log: .default, type: .debug, message)
        }

        func send(_ notification: LibboxNotification?) throws {}

        func startNeighborMonitor(_ listener: LibboxNeighborUpdateListenerProtocol?) throws {}

        func registerMyInterface(_ name: String?) {}

        func closeNeighborMonitor(_: LibboxNeighborUpdateListenerProtocol?) throws {}

        func localDNSTransport() -> (any LibboxLocalDNSTransportProtocol)? {
            nil
        }

        func systemCertificates() -> (any LibboxStringIteratorProtocol)? {
            nil
        }

        func usePlatformShell() -> Bool {
            false
        }

        func checkPlatformShell() throws {
            throw platformError("Platform shell is not supported.")
        }

        func openShellSession(
            _ user: LibboxPlatformUser?,
            command: String?,
            environ: (any LibboxStringIteratorProtocol)?,
            term: String?,
            rows: Int32,
            cols: Int32
        ) throws -> any LibboxShellSessionProtocol {
            throw platformError("Platform shell is not supported.")
        }

        func readSystemSSHHostKey(_ error: NSErrorPointer) -> String {
            error?.pointee = platformError("System SSH host key is not available.")
            return ""
        }

        func lookupSFTPServer(_ error: NSErrorPointer) -> String {
            error?.pointee = platformError("SFTP server lookup is not supported.")
            return ""
        }

        func tailscaleHostname() -> String {
            ""
        }

        func lookupUser(_ username: String?) throws -> LibboxPlatformUser {
            throw platformError("User lookup is not supported.")
        }

        func reset() {
            networkSettings = nil
            defaultPathMonitor?.cancel()
            defaultPathMonitor = nil
        }

        private func platformError(_ message: String) -> NSError {
            NSError(domain: "VoyaPacketTunnelPlatformInterface", code: -1, userInfo: [NSLocalizedDescriptionKey: message])
        }
    }

    private extension PacketTunnelProvider {
        func writeLog(_ message: String) {
            commandServer?.writeMessage(2, message: message)
        }

        func closeService() throws {
            try commandServer?.closeService()
            platformInterface.reset()
        }

        func reloadService() throws {
            let runtimeConfig = try Self.loadRuntimeConfig()
            try Self.validate(runtimeConfig)
            try commandServer?.startOrReloadService(runtimeConfig.singboxConfigJson, options: LibboxOverrideOptions())
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

    private final class VoyaNetworkInterfaceIterator: NSObject, LibboxNetworkInterfaceIteratorProtocol {
        private var iterator: IndexingIterator<[LibboxNetworkInterface]>
        private var nextValue: LibboxNetworkInterface?

        init(_ interfaces: [LibboxNetworkInterface]) {
            iterator = interfaces.makeIterator()
        }

        func hasNext() -> Bool {
            nextValue = iterator.next()
            return nextValue != nil
        }

        func next() -> LibboxNetworkInterface? {
            nextValue
        }
    }

    private func runBlocking<T>(_ block: @escaping () async throws -> T) throws -> T {
        let semaphore = DispatchSemaphore(value: 0)
        let box = VoyaResultBox<T>()
        Task.detached(priority: .userInitiated) {
            do {
                box.result = .success(try await block())
            } catch {
                box.result = .failure(error)
            }
            semaphore.signal()
        }
        semaphore.wait()
        return try box.result.get()
    }

    private func runBlocking<T>(_ block: @escaping () async -> T) -> T {
        let semaphore = DispatchSemaphore(value: 0)
        let box = VoyaResultBox<T>()
        Task.detached(priority: .userInitiated) {
            box.value = await block()
            semaphore.signal()
        }
        semaphore.wait()
        return box.value
    }

    private final class VoyaResultBox<T> {
        var result: Result<T, Error>!
        var value: T!
    }
#endif
