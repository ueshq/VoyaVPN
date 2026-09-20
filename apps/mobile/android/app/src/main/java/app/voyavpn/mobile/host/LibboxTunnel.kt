package app.voyavpn.mobile.host

import android.net.VpnService
import android.os.ParcelFileDescriptor
import io.nekohasekai.libbox.BoxService
import io.nekohasekai.libbox.InterfaceUpdateListener
import io.nekohasekai.libbox.Libbox
import io.nekohasekai.libbox.NetworkInterfaceIterator
import io.nekohasekai.libbox.PlatformInterface
import io.nekohasekai.libbox.StringIterator
import io.nekohasekai.libbox.TunOptions
import io.nekohasekai.libbox.WIFIState

/**
 * The sing-box instance behind [VoyaVpnService], and the platform it asks about.
 *
 * `openTun` is where the two halves meet: libbox hands over the tunnel it
 * wants, this builds it with the system's `VpnService.Builder`, and the file
 * descriptor goes back for libbox to read and write packets on.
 *
 * This file is compiled against the `libbox.aar`
 * `pnpm native:mobile:libbox:android` stages, and against nothing else — there
 * is no host-side build that typechecks it. Check the `PlatformInterface`
 * members against the pinned archive after a sing-box bump; the macOS
 * equivalent (`native/apple/PacketTunnel/PacketTunnelPlatform.swift`) is the
 * reference for what each one is expected to do.
 */
class LibboxTunnel(private val service: VpnService) : PlatformInterface {
    private var box: BoxService? = null
    private var descriptor: ParcelFileDescriptor? = null

    fun start(configJson: String) {
        stop()
        Libbox.setup(
            service.filesDir.absolutePath,
            service.filesDir.absolutePath,
            service.cacheDir.absolutePath,
            false,
        )
        val service = Libbox.newService(configJson, this)
        service.start()
        box = service
    }

    fun stop() {
        box?.let { runCatching { it.close() } }
        box = null
        descriptor?.let { runCatching { it.close() } }
        descriptor = null
    }

    override fun openTun(options: TunOptions): Int {
        val builder = service.Builder()
            .setSession("VoyaVPN")
            .setMtu(options.mtu)
        // libbox's iterators are Go-shaped — `hasNext`/`next`, no `Iterable` —
        // and each one has its own type, so they are walked rather than
        // adapted to a shape they do not share.
        val addresses = options.inet4Address
        while (addresses.hasNext()) {
            val address = addresses.next()
            builder.addAddress(address.address, address.prefix)
        }
        val addresses6 = options.inet6Address
        while (addresses6.hasNext()) {
            val address = addresses6.next()
            builder.addAddress(address.address, address.prefix)
        }
        if (options.autoRoute) {
            val routes = options.inet4RouteAddress
            while (routes.hasNext()) {
                val route = routes.next()
                builder.addRoute(route.address, route.prefix)
            }
            val routes6 = options.inet6RouteAddress
            while (routes6.hasNext()) {
                val route = routes6.next()
                builder.addRoute(route.address, route.prefix)
            }
            val servers = options.dnsServerAddress
            while (servers.hasNext()) {
                builder.addDnsServer(servers.next())
            }
        }
        // Per-app rules are Android-only and come later; until then the app
        // excludes itself so its own traffic — including the Clash API on
        // loopback — never re-enters the tunnel.
        builder.addDisallowedApplication(service.packageName)

        val opened = builder.establish() ?: error("the system refused to open the tunnel")
        descriptor = opened

        return opened.fd
    }

    override fun useProcFS(): Boolean = false

    override fun findConnectionOwner(
        ipProtocol: Int,
        sourceAddress: String,
        sourcePort: Int,
        destinationAddress: String,
        destinationPort: Int,
    ): Int = throw UnsupportedOperationException("connection owner lookup is not available")

    override fun packageNameByUid(uid: Int): String =
        service.packageManager.getNameForUid(uid)
            ?: throw UnsupportedOperationException("no package for uid $uid")

    override fun uidByPackageName(packageName: String): Int =
        service.packageManager.getApplicationInfo(packageName, 0).uid

    override fun usePlatformDefaultInterfaceMonitor(): Boolean = true

    override fun startDefaultInterfaceMonitor(listener: InterfaceUpdateListener) {}

    override fun closeDefaultInterfaceMonitor(listener: InterfaceUpdateListener) {}

    override fun usePlatformInterfaceGetter(): Boolean = false

    override fun getInterfaces(): NetworkInterfaceIterator =
        throw UnsupportedOperationException("interface enumeration is not available")

    override fun underNetworkExtension(): Boolean = false

    override fun includeAllNetworks(): Boolean = false

    override fun clearDNSCache() {}

    override fun readWIFIState(): WIFIState? = null

    override fun systemCertificates(): StringIterator? = null

    override fun autoDetectInterfaceControl(fd: Int) {
        // Every socket the core opens has to leave the tunnel, or its own
        // traffic would loop back through it.
        service.protect(fd)
    }

    override fun usePlatformAutoDetectInterfaceControl(): Boolean = true

    override fun writeLog(message: String) {}

    override fun sendNotification(notification: io.nekohasekai.libbox.Notification) {}
}
