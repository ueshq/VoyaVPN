package app.voyavpn.mobile.host

import android.Manifest
import android.content.Intent
import android.os.Bundle
import android.view.Gravity
import android.widget.Button
import android.widget.FrameLayout
import androidx.activity.ComponentActivity
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.barcode.BarcodeScannerOptions
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.common.InputImage
import java.util.concurrent.Executors

/** Both paths use the bundled QR decoder, including a first launch offline. */
class VoyaQrActivity : ComponentActivity() {
    private val decoder = BarcodeScanning.getClient(BarcodeScannerOptions.Builder().setBarcodeFormats(Barcode.FORMAT_QR_CODE).build())
    private val executor = Executors.newSingleThreadExecutor()
    @Volatile private var finished = false
    private var provider: ProcessCameraProvider? = null
    private val permission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) camera() else fail("cameraDenied")
    }
    // AndroidX uses the system picker when available and documents on older OS versions.
    private val picker = registerForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        if (uri == null) { finish(); return@registerForActivityResult }
        try {
            val image = InputImage.fromFilePath(this, uri)
            decoder.process(image).addOnSuccessListener { codes ->
                val values = codes.mapNotNull { it.rawValue }.distinct()
                if (values.isEmpty()) fail("noQr") else succeed(values)
            }.addOnFailureListener { fail("noQr") }
        } catch (_: Exception) { fail("noQr") }
    }
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (intent.getBooleanExtra("image", false)) {
            if (savedInstanceState == null) picker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
        } else permission.launch(Manifest.permission.CAMERA)
    }
    @androidx.annotation.OptIn(androidx.camera.core.ExperimentalGetImage::class)
    private fun camera() {
        val root = FrameLayout(this)
        val previewView = PreviewView(this)
        root.addView(previewView, FrameLayout.LayoutParams(-1, -1))
        val cancel = Button(this).apply {
            text = intent.getStringExtra("cancelLabel")
            minHeight = (48 * resources.displayMetrics.density).toInt()
            setOnClickListener { finish() }
        }
        val margin = (24 * resources.displayMetrics.density).toInt()
        val params = FrameLayout.LayoutParams(-1, -2, Gravity.BOTTOM).apply { setMargins(margin, margin, margin, margin * 3) }
        root.addView(cancel, params)
        setContentView(root)
        val future = ProcessCameraProvider.getInstance(this)
        future.addListener({
            if (isFinishing || isDestroyed) return@addListener
            try {
                val cameraProvider = future.get()
                provider = cameraProvider
                val preview = Preview.Builder().build().also { it.surfaceProvider = previewView.surfaceProvider }
                val analysis = ImageAnalysis.Builder().setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST).build()
                analysis.setAnalyzer(executor) { frame ->
                    val media = frame.image
                    if (media == null || finished) { frame.close() }
                    else {
                        decoder.process(InputImage.fromMediaImage(media, frame.imageInfo.rotationDegrees))
                            .addOnSuccessListener { codes -> codes.mapNotNull { it.rawValue }.firstOrNull()?.let { succeed(listOf(it)) } }
                            .addOnCompleteListener { frame.close() }
                    }
                }
                cameraProvider.bindToLifecycle(this, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
            } catch (_: Exception) { fail("unavailable") }
        }, ContextCompat.getMainExecutor(this))
    }
    private fun succeed(values: List<String>) {
        if (finished) return
        finished = true
        setResult(RESULT_OK, Intent().putStringArrayListExtra("values", ArrayList(values)))
        finish()
    }
    private fun fail(code: String) {
        if (finished) return
        finished = true
        setResult(RESULT_CANCELED, Intent().putExtra("error", code))
        finish()
    }
    override fun onDestroy() {
        finished = true
        provider?.unbindAll()
        decoder.close()
        executor.shutdown()
        super.onDestroy()
    }
}
