package app.voyavpn.mobile.host

import android.content.Intent
import android.net.VpnService
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.bridge.WritableMap
import com.facebook.react.bridge.Arguments
import com.facebook.react.modules.core.DeviceEventManagerModule
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import uniffi.voya_mobile_ffi.CommandException
import uniffi.voya_mobile_ffi.EventListener
import uniffi.voya_mobile_ffi.ProbeCoreException
import uniffi.voya_mobile_ffi.ProbeCoreHost
import uniffi.voya_mobile_ffi.TunnelException
import uniffi.voya_mobile_ffi.TunnelHost
import uniffi.voya_mobile_ffi.VoyaApp

/**
 * The React Native module the JS `native-transport.ts` reaches.
 *
 * The Android half of the same seam iOS's `VoyaNative` is: it owns the one
 * `VoyaApp` from `crates/voya-mobile-ffi` and supplies the three things only
 * the app can do — publish events to JS, drive [VoyaVpnService], and run a
 * probe core for a latency test while disconnected.
 *
 * Unlike iOS, the probe core is the *same* Libbox the tunnel uses, in the same
 * process. There is no extension here, so no second instance and no socket
 * path to keep apart; a disconnected run simply starts a TUN-less service of
 * its own.
 */
class VoyaNativeModule(private val context: ReactApplicationContext) :
    ReactContextBaseJavaModule(context) {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var app: VoyaApp? = null

    override fun getName(): String = NAME

    /** Required of any module React Native emits events from. */
    @ReactMethod
    fun addListener(eventName: String) = Unit

    @ReactMethod
    fun removeListeners(count: Int) = Unit

    @ReactMethod
    fun invoke(command: String, argsJson: String, promise: Promise) {
        scope.launch {
            try {
                promise.resolve(host().invoke(command, argsJson))
            } catch (error: CommandException.Rejected) {
                // The serialized AppError is the message, which is what
                // `native-transport.ts` parses back into a typed `kind`.
                promise.reject("VoyaCommandRejected", error.appErrorJson, error)
            } catch (error: Throwable) {
                promise.reject("VoyaHostUnavailable", error.message ?: error.toString(), error)
            }
        }
    }

    override fun invalidate() {
        app?.shutdown()
        app = null
        super.invalidate()
    }

    /** Starts the host on first use, so a JS reload does not reopen the database. */
    @Synchronized
    private fun host(): VoyaApp {
        app?.let { return it }
        val started = VoyaApp(
            dataDir = context.filesDir.absolutePath,
            locale = context.resources.configuration.locales[0].toLanguageTag(),
            events = JsEventListener(),
            tunnel = ServiceTunnelHost(),
            probeCore = ServiceProbeCoreHost(),
        )
        app = started

        return started
    }

    private inner class JsEventListener : EventListener {
        override fun onEvent(channel: String, payloadJson: String) {
            val payload: WritableMap = Arguments.createMap().apply {
                putString("channel", channel)
                putString("payloadJson", payloadJson)
            }
            // Dropping an event that arrives before JS is listening is not an
            // error: the frontend refetches what it needs on mount.
            if (!context.hasActiveReactInstance()) return
            context
                .getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter::class.java)
                .emit(EVENT_NAME, payload)
        }
    }

    /**
     * The tunnel, as the Rust host sees it.
     *
     * `VpnService.prepare` is the system authorization: a non-null intent
     * means the user has not granted it, and only an Activity can ask. The
     * frontend already has a seam for exactly that — `setElevationHandler` —
     * so this reports it as a permission failure rather than starting a
     * service that would be refused.
     */
    private inner class ServiceTunnelHost : TunnelHost {
        override fun start(handoffJson: String, includeAllNetworks: Boolean) {
            if (VpnService.prepare(context) != null) {
                throw TunnelException.PermissionDenied()
            }
            val intent = Intent(context, VoyaVpnService::class.java).apply {
                action = VoyaVpnService.ACTION_START
                putExtra(VoyaVpnService.EXTRA_CONFIG_JSON, handoffJson)
            }
            VoyaVpnService.lastError = null
            VoyaVpnService.state = VoyaVpnService.State.STARTING
            try {
                androidx.core.content.ContextCompat.startForegroundService(context, intent)
            } catch (error: Throwable) {
                VoyaVpnService.lastError = error.message
                VoyaVpnService.state = VoyaVpnService.State.FAILED
                throw TunnelException.Failed(error.message ?: "could not start the tunnel service")
            }
            awaitState(VoyaVpnService.State.RUNNING, START_TIMEOUT_MS)
        }

        override fun stop() {
            if (VoyaVpnService.state == VoyaVpnService.State.STOPPED) return
            VoyaVpnService.lastError = null
            VoyaVpnService.state = VoyaVpnService.State.STOPPING
            context.startService(
                Intent(context, VoyaVpnService::class.java).apply {
                    action = VoyaVpnService.ACTION_STOP
                },
            )
            awaitState(VoyaVpnService.State.STOPPED, STOP_TIMEOUT_MS)
        }

        override fun status(): String = when (VoyaVpnService.state) {
            VoyaVpnService.State.RUNNING -> "running"
            VoyaVpnService.State.STARTING, VoyaVpnService.State.STOPPING -> "starting"
            VoyaVpnService.State.STOPPED -> "stopped"
            VoyaVpnService.State.FAILED -> "error"
        }

        private fun awaitState(wanted: VoyaVpnService.State, timeoutMs: Long) {
            val deadline = System.currentTimeMillis() + timeoutMs
            while (System.currentTimeMillis() < deadline) {
                when (VoyaVpnService.state) {
                    wanted -> return
                    VoyaVpnService.State.FAILED -> throw TunnelException.Failed(
                        VoyaVpnService.lastError ?: "the tunnel service failed to start",
                    )
                    else -> Thread.sleep(POLL_INTERVAL_MS)
                }
            }
            throw TunnelException.Failed("the tunnel did not reach $wanted within ${timeoutMs}ms")
        }
    }

    /**
     * A probe core in the app process, for a latency test while disconnected.
     *
     * The same Libbox the tunnel runs, started without a TUN: the probe config
     * declares SOCKS inbounds only. While connected this is never asked —
     * `speedtest::running_core` measures through the tunnel's own Clash API,
     * which is on plain loopback here.
     */
    private inner class ServiceProbeCoreHost : ProbeCoreHost {
        private val cores = mutableMapOf<String, ProbeCore>()

        @Synchronized
        override fun start(configJson: String): String {
            val id = java.util.UUID.randomUUID().toString()
            val core = ProbeCore(context)
            try {
                core.start(configJson)
            } catch (error: Throwable) {
                throw ProbeCoreException.Failed(
                    "the probe core did not start: ${error.message ?: error}",
                )
            }
            cores[id] = core

            return id
        }

        @Synchronized
        override fun stop(coreId: String) {
            try {
                cores[coreId]?.stop()
                cores.remove(coreId)
            } catch (error: Throwable) {
                throw ProbeCoreException.Failed(error.message ?: "the probe core did not stop")
            }
        }
    }

    companion object {
        const val NAME = "VoyaNative"

        /** The one event every channel travels on; the channel is in the payload. */
        private const val EVENT_NAME = "VoyaBackendEvent"

        private const val START_TIMEOUT_MS = 60_000L
        private const val STOP_TIMEOUT_MS = 20_000L
        private const val POLL_INTERVAL_MS = 100L
    }
}
