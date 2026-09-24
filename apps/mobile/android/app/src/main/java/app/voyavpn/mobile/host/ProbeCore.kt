package app.voyavpn.mobile.host

import android.content.Context
import io.nekohasekai.libbox.CommandServer
import io.nekohasekai.libbox.Libbox
import io.nekohasekai.libbox.OverrideOptions
import io.nekohasekai.libbox.SetupOptions
import io.nekohasekai.libbox.TunOptions
import java.io.File

/** A SOCKS-only core for disconnected latency tests, using the same pinned Libbox. */
class ProbeCore(private val context: Context) : LibboxPlatform(context) {
    private var box: CommandServer? = null
    private var directory: File? = null

    fun start(configJson: String) {
        val base = File(context.cacheDir, "probe/${System.nanoTime()}").apply { mkdirs() }
        directory = base
        Libbox.setup(SetupOptions().apply {
            basePath = base.absolutePath
            workingPath = basePath
            tempPath = basePath
            logMaxLines = 500
        })
        box = Libbox.newCommandServer(this, this)
        try {
            box?.startOrReloadService(configJson, OverrideOptions())
        } catch (error: Throwable) {
            runCatching { stop() }
            throw error
        }
    }

    fun stop() {
        box?.closeService()
        box?.close()
        box = null
        closeMonitors()
        directory?.deleteRecursively()
        directory = null
    }

    override fun openTun(options: TunOptions): Int =
        throw UnsupportedOperationException("a probe core has no tunnel to open")
    override fun autoDetectInterfaceControl(fd: Int) = Unit
    override fun usePlatformAutoDetectInterfaceControl() = false
    override fun serviceStop() { Thread { stop() }.start() }
}
