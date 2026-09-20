package app.voyavpn.mobile.host

import android.content.Context
import io.nekohasekai.libbox.BoxService
import io.nekohasekai.libbox.InterfaceUpdateListener
import io.nekohasekai.libbox.Libbox
import io.nekohasekai.libbox.NetworkInterfaceIterator
import io.nekohasekai.libbox.PlatformInterface
import io.nekohasekai.libbox.StringIterator
import io.nekohasekai.libbox.TunOptions
import io.nekohasekai.libbox.WIFIState
import java.io.File

/**
 * A sing-box instance with no tunnel, for a latency test while disconnected.
 *
 * Every answer below is "there is nothing here": the probe config declares
 * SOCKS inbounds only, so libbox never opens a TUN and never looks up a
 * connection owner. Unlike [LibboxTunnel] it does not `protect` its sockets
 * either — there is no tunnel for them to loop back through.
 *
 * Working directories are per-core under `probe/`, because libbox binds
 * `<base>/command.sock` and two instances sharing a base would fight over one
 * socket.
 */
class ProbeCore(private val context: Context) : PlatformInterface {
    private var box: BoxService? = null
    private var directory: File? = null

    fun start(configJson: String) {
        val base = File(context.cacheDir, "probe/${System.nanoTime()}").apply { mkdirs() }
        directory = base
        Libbox.setup(base.absolutePath, base.absolutePath, base.absolutePath, false)
        val service = Libbox.newService(configJson, this)
        service.start()
        box = service
    }

    fun stop() {
        box?.let { runCatching { it.close() } }
        box = null
        directory?.let { runCatching { it.deleteRecursively() } }
        directory = null
    }

    override fun openTun(options: TunOptions): Int =
        throw UnsupportedOperationException("a probe core has no tunnel to open")

    override fun useProcFS(): Boolean = false

    override fun findConnectionOwner(
        ipProtocol: Int,
        sourceAddress: String,
        sourcePort: Int,
        destinationAddress: String,
        destinationPort: Int,
    ): Int = throw UnsupportedOperationException("connection owner lookup is not available")

    override fun packageNameByUid(uid: Int): String =
        throw UnsupportedOperationException("package lookup is not available")

    override fun uidByPackageName(packageName: String): Int =
        throw UnsupportedOperationException("uid lookup is not available")

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

    override fun autoDetectInterfaceControl(fd: Int) {}

    override fun usePlatformAutoDetectInterfaceControl(): Boolean = false

    override fun writeLog(message: String) {}

    override fun sendNotification(notification: io.nekohasekai.libbox.Notification) {}
}
