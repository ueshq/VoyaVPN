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
    override fun getName() = "VoyaDeviceActions"
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
        context.runOnUiQueueThread {
            try {
                val activity = context.currentActivity ?: throw IllegalStateException("No activity")
                val directory = File(context.cacheDir, "diagnostics").apply { mkdirs() }
                directory.listFiles()?.filter { System.currentTimeMillis() - it.lastModified() > 86400000 }?.forEach { it.delete() }
                val file = File(directory, "diagnostics-${UUID.randomUUID()}.txt").apply { writeText(text) }
                val uri = FileProvider.getUriForFile(context, "${context.packageName}.diagnostics", file)
                val intent = Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_STREAM, uri)
                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                intent.clipData = ClipData.newRawUri("diagnostics", uri)
                activity.startActivity(Intent.createChooser(intent, null))
                promise.resolve(null)
            } catch (error: Exception) { promise.reject("shareFailed", error) }
        }
    }
    @ReactMethod fun appVersion(promise: Promise) {
        val info = context.packageManager.getPackageInfo(context.packageName, 0)
        @Suppress("DEPRECATION") val build = info.versionCode
        promise.resolve("${info.versionName} ($build)")
    }
    companion object { private const val REQUEST_QR = 7381; private const val REQUEST_VPN = 7382 }
}
