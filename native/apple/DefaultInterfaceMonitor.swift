import Foundation
import Network

#if canImport(Libbox)
    import Libbox

    /**
     * The `NWPathMonitor` behind libbox's default-interface callbacks.
     *
     * Shared by the tunnel's platform interface (`PacketTunnel/PacketTunnelPlatform.swift`)
     * and the app's probe core (`apps/mobile/ios/VoyaVPN/Native/LibboxProbeCoreHost.swift`):
     * both answer `startDefaultInterfaceMonitor` / `closeDefaultInterfaceMonitor`, and
     * sing-box binds outbound connections to whatever this reports, so the two
     * must agree on what "the default interface" is.
     */
    final class VoyaDefaultInterfaceMonitor {
        private var monitor: NWPathMonitor?

        /// The current path once `start` has run; nil before that and after `close`.
        var currentPath: Network.NWPath? {
            monitor?.currentPath
        }

        func start(_ listener: LibboxInterfaceUpdateListenerProtocol?) {
            guard let listener else {
                return
            }

            let monitor = NWPathMonitor()
            self.monitor = monitor
            // The first path has to be in before `start` returns: libbox dials as
            // soon as the service is up, and a connection bound to no interface fails.
            let semaphore = DispatchSemaphore(value: 0)
            monitor.pathUpdateHandler = { path in
                Self.update(listener, path)
                semaphore.signal()
                monitor.pathUpdateHandler = { path in
                    Self.update(listener, path)
                }
            }
            monitor.start(queue: DispatchQueue.global(qos: .utility))
            semaphore.wait()
        }

        func close() {
            monitor?.cancel()
            monitor = nil
        }

        private static func update(_ listener: LibboxInterfaceUpdateListenerProtocol, _ path: Network.NWPath) {
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
    }
#endif
