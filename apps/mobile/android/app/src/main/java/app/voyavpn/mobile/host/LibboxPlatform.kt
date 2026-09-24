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
    private val connectivity get() = context.getSystemService(ConnectivityManager::class.java)
    private val monitors = mutableMapOf<InterfaceUpdateListener, ConnectivityManager.NetworkCallback>()

    override fun startDefaultInterfaceMonitor(listener: InterfaceUpdateListener) {
        val callback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) = publish(network, listener)
            override fun onLinkPropertiesChanged(network: Network, properties: LinkProperties) = publish(network, listener)
            override fun onCapabilitiesChanged(network: Network, capabilities: NetworkCapabilities) = publish(network, listener)
            override fun onLost(network: Network) = publish(connectivity.activeNetwork, listener)
        }
        synchronized(monitors) { monitors.put(listener, callback)?.let(connectivity::unregisterNetworkCallback) }
        connectivity.registerDefaultNetworkCallback(callback)
        publish(connectivity.activeNetwork, listener)
    }

    private fun publish(network: Network?, listener: InterfaceUpdateListener) {
        val name = network?.let(connectivity::getLinkProperties)?.interfaceName.orEmpty()
        val index = if (name.isEmpty()) -1 else JavaNetworkInterface.getByName(name)?.index ?: -1
        val capabilities = network?.let(connectivity::getNetworkCapabilities)
        listener.updateDefaultInterface(name, index,
            capabilities?.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED) == false, false)
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
