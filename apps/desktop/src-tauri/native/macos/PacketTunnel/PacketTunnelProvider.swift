import Foundation
import Network
import NetworkExtension
import os.log

#if canImport(Libbox)
    import Libbox
#endif

private let appGroupIdentifier = "group.app.voyavpn.desktop"
private let runtimeConfigRelativePath = "Library/Application Support/VoyaVPN/packet-tunnel-runtime.json"
private let providerStatusRelativePath = "Library/Application Support/VoyaVPN/packet-tunnel-status.json"
private let providerLogRelativePath = "Library/Application Support/VoyaVPN/provider.log"
private let providerLogMaxBytes = 512 * 1024
/// Bytes carried into the fresh generation when the log rotates. The host tails
/// `provider.log` for diagnostics, so a rotation must not blank the evidence.
private let providerLogCarryOverBytes = 64 * 1024

public final class PacketTunnelProvider: NEPacketTunnelProvider {
    private static let logger = Logger(subsystem: "app.voyavpn.desktop.PacketTunnel", category: "PacketTunnelProvider")
    private static var providerStatusURLOverride: URL?
    private static var providerLogURLOverride: URL?
    /// Serializes provider-log appends. libbox forwards every sing-box log line
    /// here from arbitrary Go threads, and the handle, byte counter and
    /// formatter below are shared mutable state.
    private static let providerLogQueue = DispatchQueue(label: "app.voyavpn.desktop.PacketTunnel.log")
    private static let providerLogTimestampFormatter = ISO8601DateFormatter()
    private static var providerLogHandle: FileHandle?
    private static var providerLogBytes = 0

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
                let runtimeConfig = try Self.loadRuntimeConfig(options: options)
                Self.configureDiagnostics(runtimeConfig)
                Self.writeStatus(state: "starting", breadcrumb: "startTunnel entered")
                try await self.startSingBox(runtimeConfig)
                Self.writeStatus(state: "running", breadcrumb: "sing-box service started")
                completionHandler(nil)
            } catch {
                Self.logger.error("startTunnel failed: \(error.localizedDescription, privacy: .public)")
                Self.writeStatus(
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
        Self.writeStatus(state: "stopped", breadcrumb: "stopTunnel reason \(reason.rawValue)")
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
                Self.writeStatus(state: "running", breadcrumb: "service reloaded from app message")
                completionHandler?(nil)
            #else
                throw PacketTunnelProviderError.singBoxRuntimeUnavailable
            #endif
        } catch {
            Self.writeStatus(
                state: "failed",
                lastError: error.localizedDescription,
                breadcrumb: "app message failed: \(error.localizedDescription)"
            )
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
            Self.appendProviderLog("VoyaVPN PacketTunnel started.")
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

    private static func loadRuntimeConfig(options: [String: NSObject]? = nil) throws -> PacketTunnelRuntimeConfig {
        if let data = options?["runtimeConfigJson"] as? Data {
            return try JSONDecoder().decode(PacketTunnelRuntimeConfig.self, from: data)
        }
        if let text = options?["runtimeConfigJson"] as? String,
           let data = text.data(using: .utf8)
        {
            return try JSONDecoder().decode(PacketTunnelRuntimeConfig.self, from: data)
        }
        let data = try Data(contentsOf: runtimeConfigURL())
        return try JSONDecoder().decode(PacketTunnelRuntimeConfig.self, from: data)
    }

    private static func configureDiagnostics(_ runtimeConfig: PacketTunnelRuntimeConfig) {
        providerStatusURLOverride = containedDiagnosticsURL(runtimeConfig.statusPath, kind: "status")
        providerLogURLOverride = containedDiagnosticsURL(runtimeConfig.logPath, kind: "log")
        // The cached log handle belongs to the previous destination; drop it so
        // the next line opens the one this run was configured with.
        providerLogQueue.sync { closeProviderLog() }
    }

    /// Accept a host-supplied diagnostics path only when it resolves inside the
    /// App Group container.
    ///
    /// The runtime config comes from a file any process running as the user can
    /// write, while the provider itself can run as root in the system-extension
    /// packaging, so an unchecked absolute path would let the caller pick which
    /// file the provider creates and overwrites. Anything outside the container
    /// falls back to the container default.
    private static func containedDiagnosticsURL(_ rawPath: String?, kind: String) -> URL? {
        guard let path = rawPath?.trimmingCharacters(in: .whitespacesAndNewlines), !path.isEmpty else {
            return nil
        }
        guard let containerURL = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroupIdentifier) else {
            return nil
        }

        let candidate = URL(fileURLWithPath: path).resolvingSymlinksInPath().standardizedFileURL
        let container = containerURL.resolvingSymlinksInPath().standardizedFileURL
        let containerPrefix = container.path.hasSuffix("/") ? container.path : container.path + "/"
        guard candidate.path.hasPrefix(containerPrefix) else {
            logger.error(
                "ignoring PacketTunnel \(kind, privacy: .public) path outside the App Group container: \(path, privacy: .public)"
            )
            return nil
        }

        return candidate
    }

    private static func runtimePaths() throws -> PacketTunnelRuntimePaths {
        guard let containerURL = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroupIdentifier) else {
            throw PacketTunnelProviderError.missingAppGroupContainer
        }

        // libbox binds a unix socket at "<basePath>/command.sock"; macOS caps
        // sockaddr_un.sun_path at 104 bytes (incl. NUL), so the base directory
        // must stay short. The container root plus "PT" keeps it well below
        // the limit, unlike Library/Application Support/... which exceeds it.
        let baseURL = containerURL.appendingPathComponent("PT", isDirectory: true)
        let commandSocketPath = baseURL.path + "/command.sock"
        guard commandSocketPath.utf8.count <= 103 else {
            throw PacketTunnelProviderError.libboxBasePathTooLong(commandSocketPath)
        }

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

    private static func providerStatusURL() throws -> URL {
        if let providerStatusURLOverride {
            return providerStatusURLOverride
        }
        guard let containerURL = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroupIdentifier) else {
            throw PacketTunnelProviderError.missingAppGroupContainer
        }

        return containerURL.appendingPathComponent(providerStatusRelativePath)
    }

    private static func providerLogURL() throws -> URL {
        if let providerLogURLOverride {
            return providerLogURLOverride
        }
        guard let containerURL = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroupIdentifier) else {
            throw PacketTunnelProviderError.missingAppGroupContainer
        }

        return containerURL.appendingPathComponent(providerLogRelativePath)
    }

    private static func writeStatus(
        state: String,
        lastError: String? = nil,
        breadcrumb: String
    ) {
        do {
            let url = try providerStatusURL()
            try FileManager.default.createDirectory(
                at: url.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
            let breadcrumbs = providerBreadcrumbs(from: url, appending: breadcrumb)
            var object: [String: Any] = [
                "state": state,
                "providerBundlePath": Bundle.main.bundleURL.path,
                "breadcrumbs": breadcrumbs,
                "updatedAt": ISO8601DateFormatter().string(from: Date()),
            ]
            if let lastError, !lastError.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                object["lastError"] = lastError
            }
            let data = try JSONSerialization.data(withJSONObject: object, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: url, options: .atomic)
            appendProviderLog("\(state): \(breadcrumb)")
        } catch {
            logger.error("failed to write PacketTunnel status: \(error.localizedDescription, privacy: .public)")
        }
    }

    private static func providerBreadcrumbs(from url: URL, appending breadcrumb: String) -> [String] {
        var breadcrumbs: [String] = []
        if let data = try? Data(contentsOf: url),
           let object = try? JSONSerialization.jsonObject(with: data),
           let dictionary = object as? [String: Any],
           let existing = dictionary["breadcrumbs"] as? [String]
        {
            breadcrumbs = existing
        }
        breadcrumbs.append(breadcrumb)
        return Array(breadcrumbs.suffix(20))
    }

    /// Appends one line to the provider log.
    ///
    /// This sits on the sing-box logging path inside the process that carries
    /// every packet, so it must not cost anything proportional to the file:
    /// the handle stays open and is appended to, and the file is rotated once
    /// it crosses the cap instead of being read back, trimmed and atomically
    /// rewritten per line. The write stays synchronous on a serial queue so a
    /// log burst applies backpressure instead of growing an unbounded backlog
    /// inside the tunnel process.
    fileprivate static func appendProviderLog(_ message: String) {
        let timestamp = Date()
        providerLogQueue.sync {
            writeProviderLogLine(timestamp: timestamp, message: message)
        }
    }

    private static func writeProviderLogLine(timestamp: Date, message: String) {
        let line = "\(providerLogTimestampFormatter.string(from: timestamp)) \(message)\n"
        guard let data = line.data(using: .utf8) else {
            return
        }

        do {
            let url = try providerLogURL()
            try providerLogFileHandle(url).write(contentsOf: data)
            providerLogBytes += data.count
            if providerLogBytes > providerLogMaxBytes {
                try rotateProviderLog(url)
            }
        } catch {
            // Drop the handle so the next line re-opens instead of retrying
            // against a descriptor whose file may have been replaced.
            closeProviderLog()
            logger.error("failed to write PacketTunnel log: \(error.localizedDescription, privacy: .public)")
        }
    }

    private static func providerLogFileHandle(_ url: URL) throws -> FileHandle {
        if let providerLogHandle {
            return providerLogHandle
        }

        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        if !FileManager.default.fileExists(atPath: url.path) {
            FileManager.default.createFile(atPath: url.path, contents: nil)
        }
        let handle = try FileHandle(forWritingTo: url)
        providerLogBytes = Int(try handle.seekToEnd())
        providerLogHandle = handle
        return handle
    }

    /// Moves the current generation aside and starts a new one, keeping the most
    /// recent lines so the host's log tail survives the rotation.
    private static func rotateProviderLog(_ url: URL) throws {
        closeProviderLog()
        let rotated = url.appendingPathExtension("1")
        try? FileManager.default.removeItem(at: rotated)
        try FileManager.default.moveItem(at: url, to: rotated)

        let carryOver = (try? Data(contentsOf: rotated))
            .map { Data($0.suffix(providerLogCarryOverBytes)) } ?? Data()
        FileManager.default.createFile(atPath: url.path, contents: carryOver)
    }

    /// Closes the cached handle; `providerLogBytes` is recomputed from the file
    /// the next time one is opened.
    private static func closeProviderLog() {
        try? providerLogHandle?.close()
        providerLogHandle = nil
        providerLogBytes = 0
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
    let statusPath: String?
    let logPath: String?
    let singboxConfigJson: String
}

private enum PacketTunnelProviderError: LocalizedError {
    case missingAppGroupContainer
    case emptyRuntimeConfig
    case unsupportedRuntimeConfig(Int)
    case singBoxRuntimeUnavailable
    case libboxBasePathTooLong(String)
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
            return "VoyaVPN PacketTunnel requires the sing-box Apple/libbox runtime. Build it with `pnpm native:macos:libbox` or set VOYAVPN_LIBBOX_FRAMEWORK."
        case .libboxBasePathTooLong(let path):
            return "VoyaVPN PacketTunnel libbox command socket path exceeds the macOS 104-byte sun_path limit: \(path)"
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
            let server = try options.getDNSServerAddress().value
            guard !server.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                return nil
            }

            let dnsSettings = NEDNSSettings(servers: [server])
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

        private func updateDefaultInterface(_ listener: LibboxInterfaceUpdateListenerProtocol, _ path: Network.NWPath) {
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
            PacketTunnelProvider.appendProviderLog(message)
        }

        func send(_ notification: LibboxNotification?) throws {}

        func localDNSTransport() -> (any LibboxLocalDNSTransportProtocol)? {
            nil
        }

        func systemCertificates() -> (any LibboxStringIteratorProtocol)? {
            nil
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
            Self.appendProviderLog(message)
        }

        func closeService() throws {
            try commandServer?.closeService()
            platformInterface.reset()
        }

        func reloadService() throws {
            let runtimeConfig = try Self.loadRuntimeConfig()
            try Self.validate(runtimeConfig)
            try commandServer?.startOrReloadService(runtimeConfig.singboxConfigJson, options: LibboxOverrideOptions())
            Self.writeStatus(state: "running", breadcrumb: "service reloaded")
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
