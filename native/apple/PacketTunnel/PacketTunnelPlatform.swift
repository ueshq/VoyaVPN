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
        private var networkSettings: NEPacketTunnelNetworkSettings?
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
