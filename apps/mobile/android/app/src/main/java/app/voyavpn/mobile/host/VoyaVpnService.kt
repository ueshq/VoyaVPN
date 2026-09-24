package app.voyavpn.mobile.host

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.VpnService
import android.os.Build
import androidx.core.app.NotificationCompat
import app.voyavpn.mobile.MainActivity
import app.voyavpn.mobile.R

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

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                stopTunnel()
                return START_NOT_STICKY
            }
            else -> Unit
        }

        val config = intent?.getStringExtra(EXTRA_CONFIG_JSON)
        if (config == null) {
            // Restarted by the system with no command to restart *with*: there
            // is nothing to reconnect to, and pretending otherwise would leave
            // a running service with no core in it.
            stopSelf()
            return START_NOT_STICKY
        }

        startForeground(NOTIFICATION_ID, notification())
        return try {
            val handoff = org.json.JSONObject(config)
            require(handoff.getInt("version") == 1) { "unsupported tunnel handoff version" }
            tunnel.start(handoff.getString("singboxConfigJson"))
            state = State.RUNNING
            START_STICKY
        } catch (error: Throwable) {
            lastError = error.message ?: error.toString()
            state = State.FAILED
            stopTunnel()
            START_NOT_STICKY
        }
    }

    override fun onDestroy() {
        stopTunnel()
        super.onDestroy()
    }

    /** The system revoked the VPN — another app took it, or the user did. */
    override fun onRevoke() {
        lastError = "another app took over the VPN connection"
        state = State.STOPPED
        stopTunnel()
        super.onRevoke()
    }

    private fun stopTunnel() {
        try {
            tunnel.stop()
            if (state != State.FAILED) state = State.STOPPED
        } catch (error: Throwable) {
            lastError = error.message ?: error.toString()
            state = State.FAILED
            return
        }
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
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
        const val EXTRA_CONFIG_JSON = "configJson"

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
    }
}
