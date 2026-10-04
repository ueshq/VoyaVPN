import Foundation
import Network
import NetworkExtension
import os.log

#if canImport(Libbox)
    import Libbox
#endif

#if canImport(Libbox)
    final class VoyaPacketTunnelPlatformInterface: NSObject, LibboxPlatformInterfaceProtocol, LibboxCommandServerHandlerProtocol {
        private weak var provider: PacketTunnelProvider?
        /// Written by `openTun` and `reset`, read by the DNS and proxy
        /// callbacks — libbox calls each from a thread of its own, and a stop
        /// can clear it while a network change is reading it.
        private let settingsLock = NSLock()
        private var lockedNetworkSettings: NEPacketTunnelNetworkSettings?
        private var networkSettings: NEPacketTunnelNetworkSettings? {
            get {
                settingsLock.lock()
                defer { settingsLock.unlock() }
                return lockedNetworkSettings
            }
            set {
                settingsLock.lock()
                defer { settingsLock.unlock() }
                lockedNetworkSettings = newValue
            }
        }
        private let defaultInterfaceMonitor = VoyaDefaultInterfaceMonitor()

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
            let addresses = Self.drain(options.getInet4Address())
            let settings = NEIPv4Settings(
                addresses: addresses.map { $0.address() },
                subnetMasks: addresses.map { $0.mask() }
            )
            let includedRoutes = Self.drain(options.getInet4RouteAddress()).map {
                NEIPv4Route(destinationAddress: $0.address(), subnetMask: $0.mask())
            }
            settings.includedRoutes = includedRoutes.isEmpty ? [.default()] : includedRoutes
            settings.excludedRoutes = Self.drain(options.getInet4RouteExcludeAddress()).map {
                NEIPv4Route(destinationAddress: $0.address(), subnetMask: $0.mask())
            }
            return settings
        }

        private func makeIPv6Settings(_ options: LibboxTunOptionsProtocol) -> NEIPv6Settings {
            let addresses = Self.drain(options.getInet6Address())
            let settings = NEIPv6Settings(
                addresses: addresses.map { $0.address() },
                networkPrefixLengths: addresses.map { NSNumber(value: $0.prefix()) }
            )
            let includedRoutes = Self.drain(options.getInet6RouteAddress()).map {
                NEIPv6Route(destinationAddress: $0.address(), networkPrefixLength: NSNumber(value: $0.prefix()))
            }
            settings.includedRoutes = includedRoutes.isEmpty ? [.default()] : includedRoutes
            settings.excludedRoutes = Self.drain(options.getInet6RouteExcludeAddress()).map {
                NEIPv6Route(destinationAddress: $0.address(), networkPrefixLength: NSNumber(value: $0.prefix()))
            }
            return settings
        }

        /// Everything a libbox prefix iterator holds. Libbox hands these over
        /// as optionals; a list it did not provide, or an entry it could not
        /// produce, is an empty list or a skipped entry here rather than a
        /// trap that takes the tunnel provider down mid-start.
        private static func drain(_ iterator: (any LibboxRoutePrefixIteratorProtocol)?) -> [LibboxRoutePrefix] {
            var prefixes: [LibboxRoutePrefix] = []
            while let iterator, iterator.hasNext() {
                if let prefix = iterator.next() {
                    prefixes.append(prefix)
                }
            }
            return prefixes
        }

        /// The same for a libbox string iterator.
        private static func drain(_ iterator: (any LibboxStringIteratorProtocol)?) -> [String] {
            var values: [String] = []
            while let iterator, iterator.hasNext() {
                values.append(iterator.next())
            }
            return values
        }

        private func makeProxySettings(_ options: LibboxTunOptionsProtocol) -> NEProxySettings {
            let settings = NEProxySettings()
            let server = NEProxyServer(address: options.getHTTPProxyServer(), port: Int(options.getHTTPProxyServerPort()))
            settings.httpServer = server
            settings.httpsServer = server
            settings.httpEnabled = true
            settings.httpsEnabled = true

            let bypassDomains = Self.drain(options.getHTTPProxyBypassDomain())
            if !bypassDomains.isEmpty {
                settings.exceptionList = bypassDomains
            }

            let matchDomains = Self.drain(options.getHTTPProxyMatchDomain())
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

        func startDefaultInterfaceMonitor(_ listener: LibboxInterfaceUpdateListenerProtocol?) throws {
            defaultInterfaceMonitor.start(listener)
        }

        func closeDefaultInterfaceMonitor(_: LibboxInterfaceUpdateListenerProtocol?) throws {
            defaultInterfaceMonitor.close()
        }

        func getInterfaces() throws -> LibboxNetworkInterfaceIteratorProtocol {
            guard let path = defaultInterfaceMonitor.currentPath else {
                throw platformError("Default interface monitor is not started.")
            }

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

            // Best-effort, like the two calls inside it: a cache that could
            // not be cleared is not worth failing the caller over.
            try? runBlocking {
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

        // MARK: - LibboxCommandServerHandlerProtocol
        //
        // libbox calls these four on behalf of a client of its command socket.
        // This provider never opens that socket (see `startSingBox`): the app
        // stops the tunnel through NetworkExtension and never reloads it in
        // place, and the system proxy is the host's business. They exist to
        // satisfy the protocol.

        func serviceStop() throws {}

        func serviceReload() throws {}

        func getSystemProxyStatus() throws -> LibboxSystemProxyStatus {
            LibboxSystemProxyStatus()
        }

        func setSystemProxyEnabled(_: Bool) throws {
            throw platformError("The VoyaVPN PacketTunnel sets no system proxy.")
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
            PacketTunnelDiagnostics.shared.appendProviderLog(message)
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
            defaultInterfaceMonitor.close()
        }

        private func platformError(_ message: String) -> NSError {
            NSError(domain: "VoyaPacketTunnelPlatformInterface", code: -1, userInfo: [NSLocalizedDescriptionKey: message])
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

    /// How long libbox is kept waiting on the system. The calls made through
    /// here answer in well under a second; one that never does used to hold
    /// its libbox thread for good, and with it the queue every later start
    /// and stop of the tunnel waits on.
    private let systemCallTimeout: TimeInterval = 30

    /// Runs `block` to its end on a task and hands back what it produced, for
    /// libbox callbacks, which are synchronous. Throws when the system has not
    /// answered within `systemCallTimeout`; the task is left to finish alone.
    private func runBlocking<T>(_ block: @escaping () async throws -> T) throws -> T {
        let semaphore = DispatchSemaphore(value: 0)
        let box = VoyaResultBox<T>()
        Task.detached(priority: .userInitiated) {
            do {
                box.store(.success(try await block()))
            } catch {
                box.store(.failure(error))
            }
            semaphore.signal()
        }
        guard semaphore.wait(timeout: .now() + systemCallTimeout) == .success,
              let result = box.stored()
        else {
            throw NSError(
                domain: "VoyaPacketTunnelPlatformInterface",
                code: -2,
                userInfo: [NSLocalizedDescriptionKey: "The system did not answer within \(Int(systemCallTimeout)) seconds."]
            )
        }
        return try result.get()
    }

    /// Written by the task, read by the thread that waited for it.
    private final class VoyaResultBox<T>: @unchecked Sendable {
        private let lock = NSLock()
        private var result: Result<T, Error>?

        func store(_ result: Result<T, Error>) {
            lock.lock()
            self.result = result
            lock.unlock()
        }

        func stored() -> Result<T, Error>? {
            lock.lock()
            defer { lock.unlock() }
            return result
        }
    }
#endif
