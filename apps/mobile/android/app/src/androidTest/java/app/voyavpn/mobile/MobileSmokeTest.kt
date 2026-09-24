package app.voyavpn.mobile

import android.content.Intent
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import androidx.test.uiautomator.UiScrollable
import androidx.test.uiautomator.UiSelector
import java.io.File
import org.junit.Assert.*
import org.junit.Test
import org.junit.Rule
import org.junit.rules.TestWatcher
import org.junit.runner.Description
import org.junit.runner.RunWith

/** Run against a bundled release build: no Metro, network or existing subscription required. */
@RunWith(AndroidJUnit4::class)
class MobileSmokeTest {
    private fun returnToTabs() {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        repeat(6) {
            if (device.hasObject(By.res("tab-home"))) return
            if (device.currentPackageName != InstrumentationRegistry.getInstrumentation().targetContext.packageName) return
            device.pressBack()
            device.wait(Until.hasObject(By.res("tab-home")), 1000)
        }
    }
    @get:Rule val failureEvidence = object : TestWatcher() {
        override fun finished(description: Description) { returnToTabs() }
        override fun failed(error: Throwable, description: Description) {
            val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
            capture(device, "failure-${description.methodName}")
            device.dumpWindowHierarchy(File(InstrumentationRegistry.getInstrumentation().targetContext.getExternalFilesDir(null), "failure-${description.methodName}.xml"))
        }
    }
    @Test fun languageAndThemeMatrix() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val context = instrumentation.targetContext
        val device = UiDevice.getInstance(instrumentation)
        context.startActivity(context.packageManager.getLaunchIntentForPackage(context.packageName)!!.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK))
        assertTrue(device.wait(Until.hasObject(By.res("tab-settings")), 30000))
        val languages = listOf(
            listOf("English", "Light", "Dark", "Home", "Nodes", "Rules", "Settings"),
            listOf("简体中文", "浅色", "深色", "主页", "节点", "规则", "设置"),
            listOf("繁體中文", "淺色", "深色", "主頁", "節點", "規則", "設定"))
        clickResource(device, "tab-settings")
        clickResource(device, "settings-general")
        for ((languageIndex, words) in languages.withIndex()) {
            clickLabel(device, words[0])
            for (theme in 1..2) {
                clickLabel(device, words[theme])
                device.pressBack()
                assertTrue("Return from appearance settings", device.wait(Until.hasObject(By.res("tab-home")), 10000))
                for ((index, tab) in listOf("home", "profiles", "rules", "settings").withIndex()) {
                    clickResource(device, "tab-$tab")
                    assertTrue(device.wait(Until.hasObject(By.res("page-title").text(words[index + 3])), 10000))
                    capture(device, "matrix-$languageIndex-$theme-$tab")
                }
                clickResource(device, "settings-general")
            }
        }
        clickLabel(device, "English")
        clickLabel(device, "Follow system")
        device.pressBack()
    }

    @Test fun cancellingImportPreviewDoesNotCreateNodes() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val context = instrumentation.targetContext
        val device = UiDevice.getInstance(instrumentation)
        context.startActivity(context.packageManager.getLaunchIntentForPackage(context.packageName)!!.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK))
        assertTrue(device.wait(Until.hasObject(By.res("tab-profiles")), 30000))
        val before = profileIds()
        clickResource(device, "tab-profiles")
        assertTrue(device.wait(Until.hasObject(By.res("page-title").text("Nodes")), 10000))
        clickLabel(device, "Add nodes or subscription")
        assertTrue("Import is a secondary page", device.wait(Until.gone(By.res("tab-profiles")), 10000))
        UiScrollable(UiSelector().className("android.widget.ScrollView").scrollable(true)).setSwipeDeadZonePercentage(0.25)
            .scrollIntoView(UiSelector().className("android.widget.EditText"))
        val input = device.wait(Until.findObject(By.clazz("android.widget.EditText")), 10000)
        assertNotNull(input)
        input!!.click()
        input.text = "trojan://test@cancel.example.test:443#Cancelled"
        // API 24's accessibility text replacement may leave selection actions
        // open. Dismiss that menu before scrolling the form.
        if (device.hasObject(By.desc("Cut").clazz("android.widget.Button"))) device.pressBack()
        // Dismiss only an actually visible keyboard. With a hardware keyboard,
        // unconditional Back would instead leave the import page.
        val imePackage = android.provider.Settings.Secure.getString(context.contentResolver,
            android.provider.Settings.Secure.DEFAULT_INPUT_METHOD)?.substringBefore('/')
        if (imePackage != null && device.hasObject(By.pkg(imePackage))) device.pressBack()
        clickLabel(device, "Preview")
        assertTrue(device.wait(Until.hasObject(By.textStartsWith("Found ")), 10000))
        assertTrue(revealLabel(device, "Confirm import").isEnabled)
        capture(device, "import-preview-cancel")
        device.pressBack()
        assertTrue(device.wait(Until.hasObject(By.res("tab-profiles")), 10000))
        assertEquals("Preview and cancellation must not write profiles, including offscreen rows", before, profileIds())
    }

    @Test fun vpnPermissionCancellationCanBeRetried() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val context = instrumentation.targetContext
        val device = UiDevice.getInstance(instrumentation)
        device.executeShellCommand("appops set ${context.packageName} ACTIVATE_VPN ignore")
        context.startActivity(context.packageManager.getLaunchIntentForPackage(context.packageName)!!.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        assertTrue(device.wait(Until.hasObject(By.res("tab-home")), 30000))
        val react = (context.applicationContext as MainApplication).reactHost.currentReactContext as com.facebook.react.bridge.ReactApplicationContext
        val module = app.voyavpn.mobile.host.VoyaDeviceActions(react)
        try {
            for (accept in listOf(false, true)) {
                val latch = java.util.concurrent.CountDownLatch(1)
                val answer = java.util.concurrent.atomic.AtomicReference<Boolean?>()
                module.requestVpnAuthorization(com.facebook.react.bridge.PromiseImpl(
                    com.facebook.react.bridge.Callback { values -> answer.set(values.firstOrNull() as? Boolean); latch.countDown() },
                    com.facebook.react.bridge.Callback { latch.countDown() }))
                val id = if (accept) "android:id/button1" else "android:id/button2"
                assertTrue("VPN authorization must be offered again after cancellation", device.wait(Until.hasObject(By.res(id)), 30000))
                device.findObject(By.res(id)).click()
                assertTrue(latch.await(10, java.util.concurrent.TimeUnit.SECONDS))
                assertEquals(accept, answer.get())
            }
        } finally {
            // A failed assertion must not leave a system dialog blocking the
            // next independent test.
            if (device.hasObject(By.res("android:id/alertTitle").text("Connection request"))) device.pressBack()
            module.invalidate()
        }
    }

    @Test fun bundledQrDecodesWithoutModelDownload() {
        if (InstrumentationRegistry.getArguments().getString("requireOffline") == "true") {
            val connectivity = InstrumentationRegistry.getInstrumentation().targetContext
                .getSystemService(android.net.ConnectivityManager::class.java)
            assertNull("Offline QR proof requires no active network", connectivity.activeNetwork)
        }
        val bitmap = InstrumentationRegistry.getInstrumentation().context.assets.open("offline-qr.png").use(android.graphics.BitmapFactory::decodeStream)
        val decoder = com.google.mlkit.vision.barcode.BarcodeScanning.getClient()
        try {
            val codes = com.google.android.gms.tasks.Tasks.await(
                decoder.process(com.google.mlkit.vision.common.InputImage.fromBitmap(bitmap, 0)),
                15, java.util.concurrent.TimeUnit.SECONDS)
            assertEquals("vless://77777777-7777-7777-7777-777777777777@offline.example.test:443?security=tls#Offline%20QR", codes.single().rawValue)
        } finally { decoder.close(); bitmap.recycle() }
    }

    @Test fun disconnectedProbeCoreStartsAndStops() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val port = java.net.ServerSocket(0).use { it.localPort }
        val core = app.voyavpn.mobile.host.ProbeCore(context)
        try {
            core.start("""{"log":{"level":"error"},"inbounds":[{"type":"socks","listen":"127.0.0.1","listen_port":$port}],"outbounds":[{"type":"direct"}]}""")
            java.net.Socket().use { socket ->
                socket.connect(java.net.InetSocketAddress("127.0.0.1", port), 5000)
                socket.soTimeout = 5000
                socket.getOutputStream().write(byteArrayOf(5, 1, 0))
                assertEquals(5, socket.getInputStream().read())
                assertEquals(0, socket.getInputStream().read())
            }
        } finally { core.stop() }
        assertTrue("Stopping the core releases its listener", runCatching {
            java.net.Socket().use { it.connect(java.net.InetSocketAddress("127.0.0.1", port), 500) }
        }.isFailure)
    }

    @Test fun primaryNavigationAndSecondaryBack() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val context = instrumentation.targetContext
        val device = UiDevice.getInstance(instrumentation)
        val launch = context.packageManager.getLaunchIntentForPackage(context.packageName)!!
        context.startActivity(launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK))
        assertTrue("Home tab must be reachable", device.wait(Until.hasObject(By.res("tab-home")), 30000))
        for ((tab, title) in listOf("profiles" to "Nodes", "rules" to "Rules", "settings" to "Settings", "home" to "Home")) {
            val element = device.findObject(By.res("tab-$tab"))
            assertNotNull("Missing tab $tab", element)
            element.click()
            assertTrue("Tab $tab must render its page", device.wait(Until.hasObject(By.res("page-title").text(title)), 10000))
            capture(device, "primary-$tab")
        }
        device.findObject(By.res("tab-settings")).click()
        for (page in listOf("general", "dns", "maintenance", "about")) {
            clickResource(device, "settings-$page")
            assertTrue("Secondary pages hide tabs", device.wait(Until.gone(By.res("tab-home")), 5000))
            capture(device, "secondary-$page")
            device.pressBack()
            assertTrue(device.wait(Until.hasObject(By.res("tab-home")), 5000))
        }
        device.findObject(By.res("tab-home")).click()
        clickResource(device, "home-activity")
        capture(device, "network-activity")
        assertTrue(device.wait(Until.gone(By.res("tab-home")), 5000))
        device.pressBack()
        assertTrue(device.wait(Until.hasObject(By.res("tab-home")), 5000))
    }

    private fun profileIds(): List<String> {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val react = (context.applicationContext as MainApplication).reactHost.currentReactContext as com.facebook.react.bridge.ReactApplicationContext
        val module = react.getNativeModule("VoyaNative") as? app.voyavpn.mobile.host.VoyaNativeModule
        assertNotNull("Use the running app's real Rust host", module)
        val latch = java.util.concurrent.CountDownLatch(1)
        val response = java.util.concurrent.atomic.AtomicReference<String?>()
        module!!.invoke("list_profile_summaries", "{}", com.facebook.react.bridge.PromiseImpl(
            com.facebook.react.bridge.Callback { values -> response.set(values.firstOrNull() as? String); latch.countDown() },
            com.facebook.react.bridge.Callback { latch.countDown() }))
        assertTrue(latch.await(10, java.util.concurrent.TimeUnit.SECONDS))
        assertNotNull("Profile query must succeed", response.get())
        val entries = org.json.JSONObject(response.get()!!).getJSONArray("entries")
        return (0 until entries.length()).map { entries.getJSONObject(it).getJSONObject("profile").getString("id") }.sorted()
    }

    private fun clickResource(device: UiDevice, id: String) {
        if (!id.startsWith("tab-") && device.hasObject(By.scrollable(true))) {
            val scroll = UiScrollable(UiSelector().className("android.widget.ScrollView").scrollable(true)).setMaxSearchSwipes(20)
            scroll.setSwipeDeadZonePercentage(0.25)
            scroll.scrollToBeginning(20)
            scroll.scrollIntoView(UiSelector().resourceId(id))
        }
        var element = device.findObject(By.res(id))
        // An accessibility node can exist while its center is covered by the
        // floating tabs. Scroll content actions above that bar before tapping.
        for (attempt in 0..10) {
            val tabTop = device.findObject(By.res("tab-home"))?.visibleBounds?.top ?: break
            if (id.startsWith("tab-") || element == null || element.visibleBounds.bottom < tabTop) break
            // Start above the floating surface, otherwise it consumes the
            // swipe on small screens with very large text.
            device.swipe(device.displayWidth / 2, tabTop - 64, device.displayWidth / 2, device.displayHeight / 3, 30)
            element = device.findObject(By.res(id))
        }
        assertNotNull("Missing action $id", element)
        val title = element.contentDescription
        element.click()
        device.waitForIdle()
        if (id.startsWith("tab-")) {
            assertTrue("Tab content must finish rendering before the next action",
                device.wait(Until.hasObject(By.res("page-title").text(title)), 10000))
        } else if (id.startsWith("settings-")) {
            assertTrue("Settings navigation must finish before a system Back",
                device.wait(Until.gone(By.res("tab-home")), 10000))
        }
    }

    private fun clickLabel(device: UiDevice, label: String) {
        revealLabel(device, label).click()
        device.waitForIdle()
    }

    private fun revealLabel(device: UiDevice, label: String): androidx.test.uiautomator.UiObject2 {
        fun findLabel() = device.findObject(By.desc(label).clazz("android.widget.Button")) ?: device.findObject(By.text(label)) ?: device.findObject(By.desc(label))
        var element = findLabel()
        fun reachable(): Boolean {
            val bounds = element?.visibleBounds ?: return false
            val tabTop = device.findObject(By.res("tab-home"))?.visibleBounds?.top ?: device.displayHeight
            val systemBarTop = device.findObject(By.res("com.android.systemui:id/home"))?.visibleBounds?.top ?: device.displayHeight
            val contentTop = device.findObject(By.clazz("android.widget.ScrollView"))?.visibleBounds?.top ?: 0
            // Edge-to-edge accessibility bounds include the system's three
            // navigation buttons. A label there would tap Home, not the app.
            return bounds.height() > 0 && bounds.centerY() > contentTop + 24 && bounds.bottom < minOf(tabTop, systemBarTop) - 24
        }
        // Drag the form gutter, not the center of a multiline TextInput:
        // older Android versions select/scroll text there instead of the page.
        for (forward in listOf(true, false)) {
            for (attempt in 0..12) {
                if (reachable()) break
                val bounds = device.findObject(By.clazz("android.widget.ScrollView"))?.visibleBounds ?: break
                val top = bounds.top + bounds.height() / 4
                val bottom = bounds.bottom - bounds.height() / 4
                val x = bounds.left + 8
                device.swipe(x, if (forward) bottom else top, x, if (forward) top else bottom, 30)
                device.waitForIdle()
                element = findLabel()
            }
            if (reachable()) break
        }
        assertTrue("Missing or covered choice $label", reachable())
        return element!!
    }

    private fun capture(device: UiDevice, name: String) {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val label = (InstrumentationRegistry.getArguments().getString("evidenceLabel", "standard") ?: "standard").replace(Regex("[^a-zA-Z0-9-]"), "-")
        val directory = File(context.getExternalFilesDir(null), "ux-evidence/$label").apply { mkdirs() }
        assertTrue("Screenshot $name", device.takeScreenshot(File(directory, "$name.png")))
    }
}
