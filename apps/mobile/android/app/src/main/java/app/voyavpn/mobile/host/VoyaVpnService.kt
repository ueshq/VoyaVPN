package app.voyavpn.mobile.host

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.os.Build
import android.os.Handler
import android.os.Looper
import androidx.core.app.NotificationCompat
import app.voyavpn.mobile.MainActivity
import app.voyavpn.mobile.R
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executors

/**
 * The tunnel, as a foreground service.
 *
 * Android's equivalent of iOS's PacketTunnel provider, with one difference
 * that runs through everything below: this is the *same process* as the app.
 * The Clash API is therefore reachable over plain loopback with no socket in a
 * shared container, and the core's lifetime is the service's rather than a
 * separate extension's.
 *
 * What it owns is the `VpnService` contract — the system authorization, the
 * `Builder` that opens the TUN, and the notification the OS requires of a
 * foreground service. The core itself is run by [LibboxTunnel], which is what
 * `PlatformInterface.openTun` hands the descriptor to.
 */
class VoyaVpnService : VpnService() {
    private val tunnel = LibboxTunnel(this)

    /**
     * Where the core is started and stopped.
     *
     * Every entry point below is called on the main thread, and starting the
     * core is not quick: Libbox is set up, the TUN is opened, rule sets are
     * loaded. Done inline that froze the UI for the whole start and was one
     * slow rule set away from an ANR. One thread, so a stop still runs after
     * the start it follows — `LibboxTunnel` is not synchronized and relies on
     * that order.
     */
    private val worker = Executors.newSingleThreadExecutor { runnable -> Thread(runnable, "voya-tunnel") }
    private val mainThread = Handler(Looper.getMainLooper())

    /** The id of the newest command this instance was given. Main thread only. */
    private var latestStartId = 0

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        latestStartId = startId
        when (intent?.action) {
            ACTION_STOP -> {
                stopTunnel(startId)
                return START_NOT_STICKY
            }
            else -> Unit
        }

        val config = intent?.getStringExtra(EXTRA_HANDOFF_TOKEN)?.let(pendingHandoffs::remove)
        if (config == null) {
            // Started by the system with no command to start *with*, or by a
            // start the module has since given up on: there is nothing to
            // connect to, and pretending otherwise would leave a running
            // service with no core in it.
            stopSelfResult(startId)
            return START_NOT_STICKY
        }

        // From here on this instance is the one whose tunnel the shared state
        // describes; see [report].
        current = this
        // The foreground promise has to be kept on this thread, within seconds
        // of `startForegroundService`; only the core moves off it.
        startForeground(NOTIFICATION_ID, notification())
        worker.execute {
            try {
                val handoff = org.json.JSONObject(config)
                require(handoff.getInt("version") == 1) { "unsupported tunnel handoff version" }
                tunnel.start(handoff.getString("singboxConfigJson"))
                report(State.RUNNING)
            } catch (error: Throwable) {
                report(State.FAILED, error.message ?: error.toString())
                closeTunnel(startId)
            }
        }
        // Not sticky: a restart after the process is killed arrives without
        // the configuration, so it could only start the app to stop again.
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        // Nothing to stop the service for: it is already going.
        stopTunnel(startId = null)
        // Queued work still runs; nothing new is accepted for a dead service.
        worker.shutdown()
        // A later instance takes the shared state over when it starts; until
        // one does, this instance's last close may still report.
        if (current === this) current = null
        super.onDestroy()
    }

    /**
     * The system revoked the VPN — another app took it, or the user did.
     *
     * Reported as stopped once the core has actually stopped, by the close
     * below: said here, a status read or a waiting stop would see "stopped"
     * while Libbox still held the TUN.
     */
    override fun onRevoke() {
        stopTunnel(latestStartId)
        super.onRevoke()
    }

    private fun stopTunnel(startId: Int?) {
        // `onDestroy` can follow a stop that already shut the worker down.
        runCatching { worker.execute { closeTunnel(startId) } }
    }

    /**
     * On [worker]. `startId` is the command this close answers; the service is
     * stopped for it, unless a newer command has arrived in the meantime.
     */
    private fun closeTunnel(startId: Int?) {
        try {
            tunnel.stop()
            if (state != State.FAILED) report(State.STOPPED)
        } catch (error: Throwable) {
            report(State.FAILED, error.message ?: error.toString())
            return
        }
        if (startId == null) return
        mainThread.post {
            // Reporting STOPPED above let a waiting stop return, and a
            // reconnect may already have been delivered to this instance: its
            // notification and its service are not this close's to take down.
            if (latestStartId != startId) return@post
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelfResult(startId)
        }
    }

    /**
     * Writes the shared state, for the instance that owns it.
     *
     * The state is process-wide and outlives an instance, and an instance being
     * destroyed still has a close queued on its own worker. Once a reconnect
     * has created the next instance, that late close would otherwise write
     * "stopped" over a tunnel that is starting or running.
     */
    private fun report(next: State, error: String? = null) {
        val owner = current
        if (owner != null && owner !== this) return
        if (error != null) lastError = error
        state = next
    }

    private fun notification(): Notification {
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_ID,
                    getString(R.string.app_name),
                    // Low: the tunnel's own notification is a status line, not
                    // something to interrupt anyone with.
                    NotificationManager.IMPORTANCE_LOW,
                ),
            )
        }
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle(getString(R.string.app_name))
            .setContentText(getString(R.string.vpn_notification_text))
            .setSmallIcon(R.mipmap.ic_launcher)
            .setContentIntent(open)
            .setOngoing(true)
            .build()
    }

    enum class State { STOPPED, STARTING, RUNNING, STOPPING, FAILED }

    companion object {
        const val ACTION_START = "app.voyavpn.mobile.START"
        const val ACTION_STOP = "app.voyavpn.mobile.STOP"
        private const val EXTRA_HANDOFF_TOKEN = "handoffToken"

        /**
         * Configurations on their way to the service, by the token the start
         * intent carries.
         *
         * The handshake holds the whole sing-box configuration, and a policy
         * group puts every member in it. As an intent extra that crossed
         * Binder — whose transaction buffer is about a megabyte — so a group
         * of several hundred nodes threw `TransactionTooLargeException` and
         * could never connect. The service is this process, so the text stays
         * here and only its token travels.
         */
        private val pendingHandoffs = ConcurrentHashMap<String, String>()

        /** A start intent for `handoffJson`, and the token to [forgetHandoff] it by. */
        fun startIntent(context: Context, handoffJson: String): Pair<Intent, String> {
            val token = UUID.randomUUID().toString()
            pendingHandoffs[token] = handoffJson
            val intent = Intent(context, VoyaVpnService::class.java).apply {
                action = ACTION_START
                putExtra(EXTRA_HANDOFF_TOKEN, token)
            }
            return intent to token
        }

        /** Drops a handoff the service never collected; a no-op once it has. */
        fun forgetHandoff(token: String) {
            pendingHandoffs.remove(token)
        }

        private const val CHANNEL_ID = "app.voyavpn.mobile.tunnel"
        private const val NOTIFICATION_ID = 1

        /**
         * Shared with the module rather than asked of the service.
         *
         * A bound-service round trip to read one enum would make every status
         * call asynchronous, and `NativeTunController::status` is synchronous.
         */
        @Volatile
        var state: State = State.STOPPED
            internal set

        @Volatile
        var lastError: String? = null
            internal set

        /** The instance that last took a start command, while it lives. */
        @Volatile
        private var current: VoyaVpnService? = null
    }
}
