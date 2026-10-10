package app.voyavpn.mobile.host

import android.app.Activity
import android.content.ClipData
import android.content.Intent
import androidx.core.content.FileProvider
import com.facebook.react.bridge.*
import java.io.File
import java.util.UUID

/** Native device affordances; never routes business data through another transport. */
class VoyaDeviceActions(private val context: ReactApplicationContext) : ReactContextBaseJavaModule(context) {
    private var pending: Promise? = null
    private var vpnPermission: Promise? = null
    private val listener = object : BaseActivityEventListener() {
        override fun onActivityResult(activity: Activity, requestCode: Int, resultCode: Int, data: Intent?) {
            if (requestCode == REQUEST_VPN) {
                val promise = vpnPermission
                vpnPermission = null
                promise?.resolve(resultCode == Activity.RESULT_OK && android.net.VpnService.prepare(context) == null)
                return
            }
            if (requestCode != REQUEST_QR) return
            val promise = pending ?: return
            pending = null
            val error = data?.getStringExtra("error")
            if (error != null) promise.reject(error, error)
            else if (resultCode != Activity.RESULT_OK) promise.resolve(null)
            else promise.resolve(Arguments.fromList(data?.getStringArrayListExtra("values") ?: emptyList<String>()))
        }
    }
    init { context.addActivityEventListener(listener) }
    override fun getName() = NAME
    override fun invalidate() {
        context.removeActivityEventListener(listener)
        pending?.reject("cancelled", "Host closed")
        pending = null
        vpnPermission?.resolve(false)
        vpnPermission = null
        super.invalidate()
    }
    @ReactMethod fun requestVpnAuthorization(promise: Promise) {
        context.runOnUiQueueThread {
            val activity = context.currentActivity
            if (activity == null || vpnPermission != null) { promise.resolve(false); return@runOnUiQueueThread }
            try {
                val request = android.net.VpnService.prepare(activity)
                if (request == null) { promise.resolve(true); return@runOnUiQueueThread }
                vpnPermission = promise
                activity.startActivityForResult(request, REQUEST_VPN)
            } catch (error: Exception) { vpnPermission = null; promise.reject("authorizationFailed", error) }
        }
    }
    @ReactMethod fun setConnectionShortcuts(connectLabel: String, disconnectLabel: String, promise: Promise) {
        if (android.os.Build.VERSION.SDK_INT < 25) {
            promise.reject("unsupported", "App shortcuts require Android 7.1 or later")
            return
        }
        try {
            val manager = context.getSystemService(android.content.pm.ShortcutManager::class.java)
            val shortcuts = listOf("connect" to connectLabel, "disconnect" to disconnectLabel).map { (action, label) ->
                val intent = Intent(Intent.ACTION_VIEW, android.net.Uri.parse("voyavpn://$action"), context, app.voyavpn.mobile.MainActivity::class.java)
                android.content.pm.ShortcutInfo.Builder(context, "voyavpn.$action")
                    .setShortLabel(label)
                    .setIntent(intent)
                    .build()
            }
            if (manager == null || !manager.setDynamicShortcuts(shortcuts)) {
                promise.reject("unavailable", "Connection shortcuts could not be registered")
            } else promise.resolve(null)
        } catch (error: Exception) { promise.reject("unavailable", error) }
    }
    @ReactMethod fun scanQr(cancelLabel: String, promise: Promise) = launch(false, cancelLabel, promise)
    @ReactMethod fun pickQr(promise: Promise) = launch(true, "", promise)
    private fun launch(image: Boolean, cancelLabel: String, promise: Promise) {
        context.runOnUiQueueThread {
            val activity = context.currentActivity
            if (activity == null) { promise.reject("unavailable", "No activity"); return@runOnUiQueueThread }
            if (pending != null) { promise.reject("busy", "Scanner already open"); return@runOnUiQueueThread }
            pending = promise
            try {
                activity.startActivityForResult(Intent(activity, VoyaQrActivity::class.java).putExtra("image", image).putExtra("cancelLabel", cancelLabel), REQUEST_QR)
            } catch (error: Exception) { pending = null; promise.reject("unavailable", error) }
        }
    }
    @ReactMethod fun shareDiagnostics(text: String, promise: Promise) {
        // Called on the native-modules thread. Writing the file — the whole
        // log, and the sweep of yesterday's — stays here; only showing the
        // chooser goes to the UI thread.
        val intent = try {
            val directory = File(context.cacheDir, "diagnostics").apply { mkdirs() }
            directory.listFiles()?.filter { System.currentTimeMillis() - it.lastModified() > 86400000 }?.forEach { it.delete() }
            val file = File(directory, "diagnostics-${UUID.randomUUID()}.txt").apply { writeText(text) }
            val uri = FileProvider.getUriForFile(context, "${context.packageName}.diagnostics", file)
            Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_STREAM, uri)
                .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                .apply { clipData = ClipData.newRawUri("diagnostics", uri) }
        } catch (error: Exception) { promise.reject("shareFailed", error); return }
        context.runOnUiQueueThread {
            try {
                val activity = context.currentActivity ?: throw IllegalStateException("No activity")
                activity.startActivity(Intent.createChooser(intent, null))
                promise.resolve(null)
            } catch (error: Exception) { promise.reject("shareFailed", error) }
        }
    }
    @ReactMethod fun appVersion(promise: Promise) {
        val info = context.packageManager.getPackageInfo(context.packageName, 0)
        val build = androidx.core.content.pm.PackageInfoCompat.getLongVersionCode(info)
        promise.resolve("${info.versionName} ($build)")
    }
    companion object {
        const val NAME = "VoyaDeviceActions"
        private const val REQUEST_QR = 7381
        private const val REQUEST_VPN = 7382
    }
}
