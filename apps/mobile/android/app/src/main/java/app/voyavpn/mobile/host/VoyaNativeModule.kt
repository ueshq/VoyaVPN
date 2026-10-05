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
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import uniffi.voya_mobile_ffi.CommandException
import uniffi.voya_mobile_ffi.EventListener
import uniffi.voya_mobile_ffi.ProbeCoreException
import uniffi.voya_mobile_ffi.ProbeCoreHost
import uniffi.voya_mobile_ffi.ResetException
import uniffi.voya_mobile_ffi.StartupException
import uniffi.voya_mobile_ffi.TunnelException
import uniffi.voya_mobile_ffi.TunnelHost
import uniffi.voya_mobile_ffi.VoyaApp
import uniffi.voya_mobile_ffi.resetApplicationData as resetDatabase

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

    // Both guarded by this module's monitor: commands arrive on IO threads,
    // and React Native invalidates the module from a thread of its own.
    private var app: VoyaApp? = null
    private var invalidated = false

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
            } catch (error: CancellationException) {
                // The module is going away; nobody is waiting for an answer.
                throw error
            } catch (error: CommandException.Rejected) {
                // The serialized AppError is the message, which is what
                // `native-transport.ts` parses back into a typed `kind`.
                promise.reject("VoyaCommandRejected", error.appErrorJson, error)
            } catch (error: StartupException) {
                // A startup failure carries the same serialized AppError, so
                // the frontend can branch on the typed kind: a rejected
                // database keeps its code and is the one failure the reset
                // below recovers from. Without this it reached JS as an
                // opaque message, and the reset screen never appeared.
                val appErrorJson = when (error) {
                    is StartupException.Paths -> error.appErrorJson
                    is StartupException.Database -> error.appErrorJson
                    is StartupException.Runtime -> error.appErrorJson
                }
                promise.reject("VoyaStartupFailed", appErrorJson, error)
            } catch (error: Throwable) {
                promise.reject("VoyaHostUnavailable", error.message ?: error.toString(), error)
            }
        }
    }

    /**
     * Moves the application's database aside and forgets the host, so the
     * next command reconnects against a fresh one.
     *
     * Called by the startup-failure screen the JS side renders when the
     * database was rejected; on a phone there is no shell to run a manual
     * recovery command in, so this is the only path back to a working app.
     * The iOS module does the same in `VoyaNative.swift`.
     */
    @ReactMethod
    fun resetApplicationData(promise: Promise) {
        scope.launch {
            try {
                resetWithoutHost()
                promise.resolve(null)
            } catch (error: CancellationException) {
                throw error
            } catch (error: ResetException.Failed) {
                promise.reject("VoyaResetFailed", error.appErrorJson, error)
            } catch (error: Throwable) {
                promise.reject("VoyaResetFailed", error.message ?: error.toString(), error)
            }
        }
    }

    override fun invalidate() {
        val running = synchronized(this) {
            invalidated = true
            app.also { app = null }
        }
        // Commands still queued must not outlive the module, and none may
        // build a second host: two of them racing one database is what
        // "database is locked" means.
        scope.cancel()
        running?.shutdown()
        super.invalidate()
    }

    /**
     * Closes the host, then moves the database aside, under the lock a host is
     * started with — so no command opens one in between.
     *
     * The usual caller has no host: it failed to start. But a database found
     * corrupt after a good start still has connections open on the file, and
     * moving it under them leaves them writing to the old one.
     */
    @Synchronized
    private fun resetWithoutHost() {
        // A reload that got the lock first has handed the database to this
        // module's successor, which may be opening it right now.
        check(!invalidated) { "the native module has been invalidated" }
        app.also { app = null }?.shutdown()
        resetDatabase(context.filesDir.absolutePath)
    }

    /** Starts the host on first use, so a JS reload does not reopen the database. */
    @Synchronized
    private fun host(): VoyaApp {
        check(!invalidated) { "the native module has been invalidated" }
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
            val (intent, token) = VoyaVpnService.startIntent(context, handoffJson)
            VoyaVpnService.lastError = null
            VoyaVpnService.state = VoyaVpnService.State.STARTING
            try {
                androidx.core.content.ContextCompat.startForegroundService(context, intent)
            } catch (error: Throwable) {
                VoyaVpnService.forgetHandoff(token)
                VoyaVpnService.lastError = error.message
                VoyaVpnService.state = VoyaVpnService.State.FAILED
                throw TunnelException.Failed(error.message ?: "could not start the tunnel service")
            }
            if (!awaitState(VoyaVpnService.State.RUNNING, START_TIMEOUT_MS)) {
                // A start command the system has not delivered yet finds
                // nothing to start with.
                VoyaVpnService.forgetHandoff(token)
                // The service is still trying. Left alone it could come up
                // after this has reported a failure, holding the device's
                // traffic with nothing in the app that believes a tunnel runs.
                try {
                    context.startService(
                        Intent(context, VoyaVpnService::class.java).apply {
                            action = VoyaVpnService.ACTION_STOP
                        },
                    )
                    // The start is still on the service's worker, with the
                    // stop queued behind it. Reported now, a connect made
                    // straight away would read whatever that start ends with
                    // as its own outcome — the state is shared by every
                    // attempt — so this one is over only once it has settled.
                    awaitSettled(STOP_TIMEOUT_MS)
                } catch (error: Throwable) {
                    android.util.Log.w("VoyaNative", "could not stop a tunnel that never came up", error)
                }
                throw TunnelException.Failed("the tunnel did not come up within ${START_TIMEOUT_MS}ms")
            }
        }

        override fun stop() {
            val previous = VoyaVpnService.state
            if (previous == VoyaVpnService.State.STOPPED) return
            VoyaVpnService.lastError = null
            VoyaVpnService.state = VoyaVpnService.State.STOPPING
            try {
                context.startService(
                    Intent(context, VoyaVpnService::class.java).apply {
                        action = VoyaVpnService.ACTION_STOP
                    },
                )
            } catch (error: Throwable) {
                // The command never reached the service — Android refuses a
                // background start once no foreground service is alive — so
                // nothing will ever move the state on from STOPPING.
                VoyaVpnService.state = previous
                throw TunnelException.Failed(error.message ?: "could not reach the tunnel service")
            }
            if (!awaitState(VoyaVpnService.State.STOPPED, STOP_TIMEOUT_MS)) {
                throw TunnelException.Failed("the tunnel was still up after ${STOP_TIMEOUT_MS}ms")
            }
        }

        override fun status(): String = when (VoyaVpnService.state) {
            VoyaVpnService.State.RUNNING -> "running"
            VoyaVpnService.State.STARTING, VoyaVpnService.State.STOPPING -> "starting"
            VoyaVpnService.State.STOPPED -> "stopped"
            VoyaVpnService.State.FAILED -> "error"
        }

        /** `false` when the time ran out; a service that failed throws its reason. */
        private fun awaitState(wanted: VoyaVpnService.State, timeoutMs: Long): Boolean {
            val deadline = System.currentTimeMillis() + timeoutMs
            while (System.currentTimeMillis() < deadline) {
                when (VoyaVpnService.state) {
                    wanted -> return true
                    VoyaVpnService.State.FAILED -> throw TunnelException.Failed(
                        VoyaVpnService.lastError ?: "the tunnel service failed to start",
                    )
                    else -> Thread.sleep(POLL_INTERVAL_MS)
                }
            }
            return false
        }

        /** Waits, for at most `timeoutMs`, until nothing is starting, running or stopping. */
        private fun awaitSettled(timeoutMs: Long) {
            val deadline = System.currentTimeMillis() + timeoutMs
            while (System.currentTimeMillis() < deadline) {
                when (VoyaVpnService.state) {
                    VoyaVpnService.State.STOPPED, VoyaVpnService.State.FAILED -> return
                    else -> Thread.sleep(POLL_INTERVAL_MS)
                }
            }
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
            // Forgotten whether or not it stops cleanly: nothing asks for a
            // probe core a second time, so one kept here after a failed stop
            // would be held for the life of the process.
            val core = cores.remove(coreId) ?: return
            try {
                core.stop()
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
