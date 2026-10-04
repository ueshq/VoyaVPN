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

    @Synchronized
    fun start(configJson: String) {
        val base = File(context.cacheDir, "probe/${System.nanoTime()}").apply { mkdirs() }
        directory = base
        try {
            synchronized(setupLock) {
                Libbox.setup(SetupOptions().apply {
                    basePath = base.absolutePath
                    workingPath = basePath
                    tempPath = basePath
                    // Libbox's own log buffer is for a command client; nothing
                    // connects one, so it would only hold lines nobody reads.
                    logMaxLines = 0
                })
                box = Libbox.newCommandServer(this, this)
                box?.startOrReloadService(configJson, OverrideOptions())
            }
        } catch (error: Throwable) {
            // Whatever got as far as existing — the directory at least — goes.
            runCatching { stop() }
            throw error
        }
    }

    /**
     * Safe to call twice and from two threads: the host stops a probe core
     * when its test ends, and Libbox may ask for the same through
     * [serviceStop] on a thread of its own.
     */
    @Synchronized
    fun stop() {
        try {
            box?.closeService()
        } finally {
            // The host forgets a probe core whether or not it stops cleanly,
            // so a failed stop is the last chance to let go of the server, the
            // network callbacks and the directory.
            runCatching { box?.close() }
            box = null
            closeMonitors()
            directory?.deleteRecursively()
            directory = null
        }
    }

    override fun openTun(options: TunOptions): Int =
        throw UnsupportedOperationException("a probe core has no tunnel to open")
    override fun autoDetectInterfaceControl(fd: Int) = Unit
    override fun usePlatformAutoDetectInterfaceControl() = false
    override fun serviceStop() { Thread { stop() }.start() }
}
