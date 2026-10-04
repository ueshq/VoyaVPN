package app.voyavpn.mobile.host

import android.content.Context
import android.net.ConnectivityManager
import android.net.LinkProperties
import android.net.Network
import android.net.NetworkCapabilities
import android.system.OsConstants
import android.util.Log
import io.nekohasekai.libbox.*
import java.net.NetworkInterface as JavaNetworkInterface

/** Platform facts shared by the tunnel and the disconnected latency core. */
abstract class LibboxPlatform(private val context: Context) : PlatformInterface, CommandServerHandler {
    companion object {
        /**
         * Held from `Libbox.setup` until the service it prepares has started.
         *
         * `setup` writes the base, working and temp paths into Go globals,
         * and `startOrReloadService` reads them. The tunnel and a probe core
         * live in one process, on different threads: a probe setting up in
         * between would have the tunnel start in the probe's directory, which
         * the probe deletes when its test ends.
         */
        val setupLock = Any()
    }

    private val connectivity get() = context.getSystemService(ConnectivityManager::class.java)
    private val monitors = mutableMapOf<InterfaceUpdateListener, ConnectivityManager.NetworkCallback>()

    override fun startDefaultInterfaceMonitor(listener: InterfaceUpdateListener) {
        val callback = object : ConnectivityManager.NetworkCallback() {
            /** The network and metered flag last published; the system calls these one at a time. */
            private var published: Pair<Network?, Boolean>? = null

            private fun report(network: Network?) {
                published = network to publish(network, listener)
            }

            override fun onAvailable(network: Network) = report(network)
            override fun onLinkPropertiesChanged(network: Network, properties: LinkProperties) = report(network)
            override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) {
                // Signal strength arrives here too, every few seconds, and
                // each update has the core list every interface again. The
                // metered flag is the only capability it is told.
                if (published != network to isMetered(capabilities)) report(network)
            }
            override fun onLost(network: Network) = report(connectivity.activeNetwork)
        }
        synchronized(monitors) { monitors.put(listener, callback)?.let(connectivity::unregisterNetworkCallback) }
        connectivity.registerDefaultNetworkCallback(callback)
        publish(connectivity.activeNetwork, listener)
    }

    private fun isMetered(capabilities: NetworkCapabilities?) =
        capabilities?.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED) == false

    /** Tells the core the default interface and returns the metered flag it was told. */
    private fun publish(network: Network?, listener: InterfaceUpdateListener): Boolean {
        val name = network?.let(connectivity::getLinkProperties)?.interfaceName.orEmpty()
        // `getByName` throws when the interface list cannot be read, and this
        // runs on the system's callback thread, where that would end the
        // process hosting the tunnel.
        val index = if (name.isEmpty()) -1 else runCatching { JavaNetworkInterface.getByName(name)?.index }.getOrNull() ?: -1
        val metered = isMetered(network?.let(connectivity::getNetworkCapabilities))
        listener.updateDefaultInterface(name, index, metered, false)
        return metered
    }

    override fun closeDefaultInterfaceMonitor(listener: InterfaceUpdateListener) {
        synchronized(monitors) { monitors.remove(listener) }?.let(connectivity::unregisterNetworkCallback)
    }

    fun closeMonitors() {
        val callbacks = synchronized(monitors) { monitors.values.toList().also { monitors.clear() } }
        callbacks.forEach { runCatching { connectivity.unregisterNetworkCallback(it) } }
    }

    override fun getInterfaces(): NetworkInterfaceIterator {
        val networks = connectivity.allNetworks.mapNotNull { network ->
            connectivity.getLinkProperties(network)?.let { it to connectivity.getNetworkCapabilities(network) }
        }
        val interfaces = JavaNetworkInterface.getNetworkInterfaces()?.toList().orEmpty().map { item ->
            val (properties, capabilities) = networks.firstOrNull { it.first.interfaceName == item.name } ?: (null to null)
            io.nekohasekai.libbox.NetworkInterface().apply {
                name = item.name
                index = item.index
                mtu = item.mtu
                addresses = stringIterator(item.interfaceAddresses.map { "${it.address.hostAddress?.substringBefore('%')}/${it.networkPrefixLength}" })
                dnsServer = stringIterator(properties?.dnsServers.orEmpty().mapNotNull { it.hostAddress })
                metered = capabilities?.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED) == false
                type = when {
                    capabilities?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true -> Libbox.InterfaceTypeWIFI
                    capabilities?.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) == true -> Libbox.InterfaceTypeCellular
                    capabilities?.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) == true -> Libbox.InterfaceTypeEthernet
                    else -> Libbox.InterfaceTypeOther
                }
                flags = (if (item.isUp) OsConstants.IFF_UP or OsConstants.IFF_RUNNING else 0) or
                    (if (item.isLoopback) OsConstants.IFF_LOOPBACK else 0) or
                    (if (item.isPointToPoint) OsConstants.IFF_POINTOPOINT else 0) or
                    (if (item.supportsMulticast()) OsConstants.IFF_MULTICAST else 0)
            }
        }.iterator()
        return object : NetworkInterfaceIterator {
            override fun hasNext() = interfaces.hasNext()
            override fun next() = interfaces.next()
        }
    }

    private fun stringIterator(values: List<String>): StringIterator {
        val iterator = values.iterator()
        return object : StringIterator {
            override fun hasNext() = iterator.hasNext()
            override fun next() = iterator.next()
            override fun len() = values.size
        }
    }

    override fun useProcFS() = false
    override fun findConnectionOwner(ipProtocol: Int, sourceAddress: String, sourcePort: Int,
        destinationAddress: String, destinationPort: Int): ConnectionOwner =
        throw UnsupportedOperationException("connection owner lookup is not available")
    override fun underNetworkExtension() = false
    override fun includeAllNetworks() = false
    override fun clearDNSCache() = Unit
    override fun localDNSTransport(): LocalDNSTransport? = null
    override fun readWIFIState(): WIFIState? = null
    override fun systemCertificates(): StringIterator? = null
    override fun sendNotification(notification: io.nekohasekai.libbox.Notification) = Unit
    override fun getSystemProxyStatus() = SystemProxyStatus()
    override fun setSystemProxyEnabled(enabled: Boolean) {
        if (enabled) throw UnsupportedOperationException("Android uses the VPN tunnel")
    }
    override fun serviceReload(): Unit = throw UnsupportedOperationException("reload through the app")
    override fun writeDebugMessage(message: String) { Log.d("VoyaLibbox", message) }
}
