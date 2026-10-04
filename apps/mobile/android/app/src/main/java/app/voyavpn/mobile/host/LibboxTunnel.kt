package app.voyavpn.mobile.host

import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.ParcelFileDescriptor
import io.nekohasekai.libbox.CommandServer
import io.nekohasekai.libbox.Libbox
import io.nekohasekai.libbox.OverrideOptions
import io.nekohasekai.libbox.SetupOptions
import io.nekohasekai.libbox.TunOptions

/** Owns the pinned Libbox service and the descriptor returned by Android's VPN API. */
class LibboxTunnel(private val service: VpnService) : LibboxPlatform(service) {
    private var box: CommandServer? = null
    private var descriptor: ParcelFileDescriptor? = null

    fun start(configJson: String) {
        stop()
        synchronized(setupLock) {
            Libbox.setup(SetupOptions().apply {
                basePath = service.filesDir.absolutePath
                workingPath = basePath
                tempPath = service.cacheDir.absolutePath
                // Libbox's own log buffer is for a command client; nothing
                // connects one, so it would only hold lines nobody reads.
                logMaxLines = 0
            })
            val core = Libbox.newCommandServer(this, this)
            box = core
            try {
                // start() exposes a separate command listener; the app uses the Clash API.
                core.startOrReloadService(configJson, OverrideOptions())
            } catch (error: Throwable) {
                runCatching { stop() }
                throw error
            }
        }
    }

    fun stop() {
        // Do not report success if the core refused to stop: callers must retain
        // nodes/configuration until disconnection has actually completed.
        box?.closeService()
        box?.close()
        box = null
        closeMonitors()
        descriptor?.close()
        descriptor = null
    }

    override fun openTun(options: TunOptions): Int {
        val builder = service.Builder().setSession("VoyaVPN").setMtu(options.mtu)
        // A VPN is metered unless it says otherwise, and every app would then
        // hold its updates and backups for as long as this one is connected.
        // Unset, it takes after the network underneath.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) builder.setMetered(false)
        val addresses = options.inet4Address
        while (addresses.hasNext()) addresses.next().let { builder.addAddress(it.address(), it.prefix()) }
        val addresses6 = options.inet6Address
        while (addresses6.hasNext()) addresses6.next().let { builder.addAddress(it.address(), it.prefix()) }
        if (options.autoRoute) {
            // RouteRange also expands exclusions on Android versions before API 33.
            val routes = options.inet4RouteRange
            while (routes.hasNext()) routes.next().let { builder.addRoute(it.address(), it.prefix()) }
            val routes6 = options.inet6RouteRange
            while (routes6.hasNext()) routes6.next().let { builder.addRoute(it.address(), it.prefix()) }
            options.dnsServerAddress?.value?.takeIf { it.isNotBlank() }?.let(builder::addDnsServer)
        }
        builder.addDisallowedApplication(service.packageName)
        val opened = builder.establish() ?: error("the system refused to open the tunnel")
        descriptor = opened
        return opened.fd
    }

    override fun autoDetectInterfaceControl(fd: Int) {
        check(service.protect(fd)) { "the system refused to protect a core socket" }
    }
    override fun usePlatformAutoDetectInterfaceControl() = true
    override fun serviceStop() {
        service.startService(Intent(service, VoyaVpnService::class.java).setAction(VoyaVpnService.ACTION_STOP))
    }
}
