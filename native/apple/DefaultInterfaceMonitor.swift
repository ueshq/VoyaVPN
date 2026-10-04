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
        /// How long `start` waits for the first path. The system reports one
        /// almost at once; the bound is there so that a monitor that never
        /// does cannot hold the tunnel's start — and the thread libbox called
        /// in on — forever. A path that arrives later is still delivered.
        private static let firstPathTimeout: DispatchTimeInterval = .seconds(5)

        /// `start` and `close` come from libbox's threads and `currentPath`
        /// from whichever thread asks, so the monitor is read under a lock.
        private let lock = NSLock()
        private var monitor: NWPathMonitor?
        private let queue = DispatchQueue(label: "VoyaDefaultInterfaceMonitor", qos: .utility)

        /// The current path once `start` has run; nil before that and after `close`.
        var currentPath: Network.NWPath? {
            lock.lock()
            defer { lock.unlock() }
            return monitor?.currentPath
        }

        func start(_ listener: LibboxInterfaceUpdateListenerProtocol?) {
            guard let listener else {
                return
            }

            let monitor = NWPathMonitor()
            lock.lock()
            let previous = self.monitor
            self.monitor = monitor
            lock.unlock()
            // A second start without a close would leave the first monitor
            // reporting to a listener nobody holds any more.
            previous?.cancel()

            // The first path has to be in before `start` returns: libbox dials as
            // soon as the service is up, and a connection bound to no interface fails.
            // One handler for every update; the queue is serial, so the flag
            // needs no lock of its own.
            let semaphore = DispatchSemaphore(value: 0)
            var signalled = false
            monitor.pathUpdateHandler = { path in
                Self.update(listener, path)
                if !signalled {
                    signalled = true
                    semaphore.signal()
                }
            }
            monitor.start(queue: queue)
            _ = semaphore.wait(timeout: .now() + Self.firstPathTimeout)
        }

        func close() {
            lock.lock()
            let monitor = self.monitor
            self.monitor = nil
            lock.unlock()
            monitor?.cancel()
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
