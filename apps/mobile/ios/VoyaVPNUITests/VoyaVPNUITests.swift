import XCTest
import Vision
import UIKit

/// Release app, real native host, no mock transport or production test hooks.
/// Every flow imports its own fixture; no test depends on another test's data.
final class VoyaVPNUITests: XCTestCase {
    private var app: XCUIApplication!
    private let timeout: TimeInterval = 30

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        addUIInterruptionMonitor(withDescription: "Clipboard permission") { alert in
            let allow = alert.buttons["Allow Paste"]
            guard allow.exists else { return false }
            allow.tap()
            return true
        }
        launch()
        open("general")
        visible(row("English")).tap()
        let system = app.buttons["Follow system"]
        XCTAssertTrue(system.waitForExistence(timeout: timeout))
        visible(system).tap()
        open("home")
    }

    override func tearDownWithError() throws {
        capture(name)
        let tree = XCTAttachment(string: app.debugDescription)
        tree.name = name + "-accessibility"
        tree.lifetime = .keepAlways
        add(tree)
        app.terminate()
    }

    func testDailyConnectionNavigation() throws {
        open("profiles")
        try importText("vless://33333333-3333-3333-3333-333333333333@daily.example.test:443?security=tls#Daily%20Node")
        row("Daily Node").tap()
        open("home")
        tap("Daily Node")
        XCTAssertTrue(app.searchFields["Search nodes"].waitForExistence(timeout: timeout))
        XCTAssertFalse(app.buttons["Test all"].exists)
        XCTAssertFalse(app.buttons["Add nodes or subscription"].exists)
        captureStable("daily-node-picker")
        row("Daily Node").tap()
        XCTAssertTrue(app.buttons["Connect"].waitForExistence(timeout: timeout))
        XCTAssertFalse(app.buttons["Connection details"].exists)
        open("dns")
        XCTAssertTrue(app.switches["Connect when the app starts"].exists)
        tap("Connection shortcuts")
        tap("Set up shortcuts")
        XCTAssertTrue(app.staticTexts["Shortcuts are ready."].waitForExistence(timeout: timeout))
        captureStable("daily-connection-options")
    }

    func testLaunchAndAllPages() {
        open("profiles")
        XCTAssertFalse(row("🇯🇵 Tokyo").exists, "the mock backend must not be installed")
        for page in ["home", "profiles", "rules", "settings"] {
            open(page)
            capture(page)
            XCUIDevice.shared.press(.home)
            app.activate()
            XCTAssertTrue(app.wait(for: .runningForeground, timeout: timeout))
            // Selection alone is already true while SpringBoard is still
            // restoring the window. Wait for the rendered page to settle
            // before a subsequent tab tap can be treated as an app action.
            captureStable("restored-\(page)")
            XCTAssertTrue(wait { self.tabButton(page).isSelected })
        }
        open("connections")
        XCTAssertTrue(app.staticTexts["Connect to view network activity"].exists)
        XCTAssertFalse(tabButton("home").isHittable)
        open("home")
        XCTAssertTrue(tabButton("home").isSelected)
    }

    func testNodeImportShareQrDelete() throws {
        let title = "QA Lifecycle"
        open("profiles")
        try importText("")
        XCTAssertFalse(app.buttons["Preview"].isEnabled)
        try importText("not-a-node")
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS 'No importable'")).firstMatch.waitForExistence(timeout: timeout))
        let link = "vless://22222222-2222-2222-2222-222222222222@qa.example.test:443?security=tls#QA%20Lifecycle"
        try importText(link)
        XCTAssertTrue(row(title).waitForExistence(timeout: timeout))
        try importText(link)
        XCTAssertEqual(app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", title)).count, 1)
        row(title).press(forDuration: 1.2)
        _ = try fixture("clipboard", body: "QA export pending")
        tap("Copy link")
        XCTAssertTrue(wait { !self.app.buttons["Copy link"].exists })
        let exported = try exportedLink(containing: "qa.example.test")
        row(title).press(forDuration: 1.2)
        tap("Show QR")
        let qr = app.images["Generated QR code"]
        XCTAssertTrue(qr.waitForExistence(timeout: timeout))
        assertQr(qr, equals: exported)
        capture("decoded-qr")
        tap("Close")
        row(title).tap()
        relaunch()
        open("profiles")
        XCTAssertTrue(row(title).isSelected)
        open("home")
        tap("Connect")
        // A simulator cannot grant the VPN configuration, so the connect ends
        // as a declined authorization — which Home answers with a way to ask
        // again, and no diagnostic that would only repeat the sentence above it.
        XCTAssertTrue(app.buttons["Authorize again"].waitForExistence(timeout: timeout))
        tap("Authorize again")
        XCTAssertTrue(app.staticTexts["Disconnected"].exists)
        open("profiles")
        row(title).press(forDuration: 1.2)
        tap("Delete")
        XCTAssertTrue(app.alerts.firstMatch.waitForExistence(timeout: timeout))
        app.alerts.buttons["Delete"].tap()
        XCTAssertTrue(wait { !self.row(title).exists })
        relaunch()
        open("profiles")
        XCTAssertFalse(row(title).exists)
    }

    func testSubscriptionAutoRefreshAndPolicyGroup() throws {
        open("profiles")
        try importText(required("QA_SUBSCRIPTION_URL"))
        XCTAssertTrue(row("QA Subscription A").waitForExistence(timeout: timeout))
        XCTAssertTrue(row("QA Subscription B").exists)
        tap("Policy groups")
        let group = app.buttons.matching(NSPredicate(format: "label CONTAINS 'Auto' AND NOT (label CONTAINS 'tab')")).firstMatch
        XCTAssertTrue(group.waitForExistence(timeout: timeout))
        group.tap()
        XCTAssertTrue(wait { group.isSelected })
        relaunch()
        open("profiles")
        tap("Policy groups")
        XCTAssertTrue(group.isSelected)
        open("subscriptions")
        tap("Update all subscriptions")
        open("profiles")
        XCTAssertTrue(row("QA Subscription Updated").waitForExistence(timeout: timeout))
        open("subscriptions")
        tap("Update all subscriptions")
        open("profiles")
        XCTAssertTrue(row("QA Subscription Refreshed").waitForExistence(timeout: timeout))
        visible(row("QA Subscription Refreshed")).press(forDuration: 1.2)
        XCTAssertTrue(app.buttons["Copy link"].waitForExistence(timeout: timeout))
        XCTAssertTrue(app.buttons["Show QR"].exists)
        XCTAssertFalse(app.buttons["Delete"].exists)
        capture("subscription-read-only")
        tap("Close")
    }

    func testRealLatencyTimeoutCancelAndRetry() throws {
        open("profiles")
        let port = required("QA_VLESS_PORT")
        try importText("vless://44444444-4444-4444-4444-444444444444@127.0.0.1:\(port)?security=none#QA%20Latency")
        XCTAssertTrue(row("QA Latency").waitForExistence(timeout: timeout))
        _ = try fixture("delay", body: "0")
        tap("Test all")
        XCTAssertTrue(wait { self.row("QA Latency").label.contains(" ms") }, "a real probe must return a successful measurement")
        _ = try fixture("delay", body: "10000")
        defer { _ = try? fixture("delay", body: "0") }
        tap("Test all")
        let stop = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Stop'")).firstMatch
        XCTAssertTrue(ready(stop))
        stop.tap()
        XCTAssertTrue(ready(app.buttons["Test all"]))
        tap("Test all")
        XCTAssertTrue(wait { self.row("QA Latency").label.lowercased().contains("timed out") })
        _ = try fixture("delay", body: "0")
        tap("Test all")
        XCTAssertTrue(wait { self.row("QA Latency").label.contains(" ms") })
        capture("real-latency-success")
    }

    func testRulesAndSettingsPersist() {
        open("rules")
        tap("Rule")
        var values: [String: String] = [:]
        for label in ["AI services via proxy", "Block QUIC (UDP 443)", "China public DNS direct", "Bypass LAN", "Block ads", "China sites direct"] {
            let control = visible(app.switches[label])
            XCTAssertTrue(control.exists)
            let before = control.value as? String
            control.tap()
            XCTAssertTrue(wait { control.value as? String != before })
            values[label] = control.value as? String
        }
        tap("Global")
        XCTAssertFalse(app.switches["AI services via proxy"].isEnabled)
        relaunch()
        open("rules")
        XCTAssertTrue(app.buttons["Global"].isSelected)
        tap("Rule")
        for (label, value) in values { XCTAssertEqual(visible(app.switches[label]).value as? String, value) }
        open("maintenance")
        let label = "Record detailed connection log"
        let control = visible(app.switches[label])
        let before = control.value as? String
        control.tap()
        XCTAssertTrue(wait { control.value as? String != before })
        let expected = control.value as? String
        relaunch()
        open("maintenance")
        XCTAssertEqual(visible(app.switches[label]).value as? String, expected)
        open("general")
        tap("Dark")
        relaunch()
        open("general")
        XCTAssertTrue(app.buttons["Dark"].isSelected)
    }

    func testDnsValidationAndPersistence() {
        open("dns")
        tap("Custom & advanced")
        let field = visible(app.textFields["Bootstrap DNS"])
        XCTAssertTrue(field.exists)
        let original = field.value as? String ?? ""
        field.tap()
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: original.count) + "1.1.1.1:99999\n")
        tap("Save")
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS '65535' OR label CONTAINS '65,535' OR label CONTAINS '99999'")).firstMatch.waitForExistence(timeout: timeout))
        field.tap()
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: "1.1.1.1:99999".count) + "1.1.1.1\n")
        tap("Save")
        capture("dns-save-feedback")
        XCTAssertTrue(app.staticTexts["Saved"].waitForExistence(timeout: timeout))
        relaunch()
        open("dns")
        tap("Custom & advanced")
        XCTAssertEqual(app.textFields["Bootstrap DNS"].value as? String, "1.1.1.1")
    }

    func testSecondaryPagesAndImportCancellation() throws {
        for page in ["subscriptions", "connections", "dns", "maintenance", "logs", "about"] {
            open(page)
            captureStable(page)
            XCTAssertFalse(tabButton("home").isHittable)
        }
        open("profiles")
        // Through the clipboard, like every other import here: XCUITest does
        // not see keyboard focus on the multiline field, so it cannot type
        // into it.
        try readClipboard("trojan://test@cancel.example.test:443#Cancelled")
        tap("Preview")
        XCTAssertTrue(app.buttons["Confirm import"].waitForExistence(timeout: timeout))
        open("profiles")
        XCTAssertFalse(row("Cancelled").exists)
    }

    func testSystemThemeAndForegroundRecovery() throws {
        let original = String(data: try fixture("appearance"), encoding: .utf8) ?? "light"
        defer { _ = try? fixture("appearance", body: original) }
        open("general")
        _ = try fixture("appearance", body: "light")
        tap("Light")
        let light = backgroundPixel()
        tap("Dark")
        let dark = backgroundPixel()
        XCTAssertNotEqual(light, dark, "Explicit themes must produce different backgrounds")
        tap("Follow system")
        XCTAssertTrue(wait { self.backgroundPixel() == light }, "Follow system must clear the dark override")
        _ = try fixture("appearance", body: "dark")
        XCTAssertTrue(wait { self.backgroundPixel() == dark }, "System changes must update the app")
        XCUIDevice.shared.press(.home)
        _ = try fixture("appearance", body: "light")
        app.activate()
        XCTAssertTrue(wait { self.backgroundPixel() == light }, "Appearance must refresh after foregrounding")
        capture("system-theme-restored")
    }

    private func backgroundPixel() -> [UInt8] {
        guard let image = app.screenshot().image.cgImage,
              image.bitsPerPixel == 32,
              let data = image.dataProvider?.data,
              let bytes = CFDataGetBytePtr(data) else { return [] }
        // The page gutter is canvas in all supported phone/tablet layouts.
        let scale = Double(image.width) / app.frame.width
        let offset = Int(200 * scale) * image.bytesPerRow + Int(2 * scale) * 4
        return Array(UnsafeBufferPointer(start: bytes + offset, count: 3))
    }

    func testTabletOrientations() throws {
        try XCTSkipUnless(UIDevice.current.userInterfaceIdiom == .pad, "Tablet directions are iPad-only")
        defer { XCUIDevice.shared.orientation = .portrait }
        for orientation in [UIDeviceOrientation.portrait, .landscapeLeft, .landscapeRight, .portraitUpsideDown] {
            XCUIDevice.shared.orientation = orientation
            let landscape = orientation == .landscapeLeft || orientation == .landscapeRight
            XCTAssertTrue(wait { (self.app.frame.width > self.app.frame.height) == landscape })
            for page in ["home", "profiles", "rules", "settings"] {
                open(page)
                XCTAssertTrue(tabButton(page).isHittable)
                captureStable("ipad-\(orientation.rawValue)-\(page)")
            }
        }
    }

    func testVisualMatrix() throws {
        open("profiles")
        let batch = (0..<120).map { index in
            "vless://77777777-7777-7777-7777-777777777777@\(index).visual.example.test:443?security=tls#QA%20Visual%20\(index)%20-%20Long%20international%20node%20name%20for%20accessibility"
        }.joined(separator: "\n")
        try importText(batch)
        for (language, _) in [("English", "en"), ("简体中文", "zh-Hans"), ("繁體中文", "zh-Hant")] {
            open("general")
            visible(row(language)).tap()
            for dark in [false, true] {
                open("general")
                let label = language == "English" ? (dark ? "Dark" : "Light") : language == "简体中文" ? (dark ? "深色" : "浅色") : (dark ? "深色" : "淺色")
                visible(app.buttons[label]).tap()
                for page in ["home", "profiles", "rules", "settings"] {
                    open(page)
                    captureStable("\(language)-\(dark ? "dark" : "light")-\(page)")
                    if page == "profiles" {
                        visible(row("QA Visual")).press(forDuration: 1.2)
                        let showQr = language == "English" ? "Show QR" : language == "简体中文" ? "显示二维码" : "顯示 QR Code"
                        let close = language == "English" ? "Close" : language == "简体中文" ? "关闭" : "關閉"
                        // Small phones with accessibility text legitimately
                        // scroll the actions. Reveal the row before checking
                        // reachability, just as the subsequent tap does.
                        XCTAssertTrue(ready(visible(app.buttons[showQr])))
                        captureStable("\(language)-\(dark)-actions")
                        let share = language == "English" ? "Copy link" : language == "简体中文" ? "复制链接" : "複製連結"
                        // A previous import/export must not make a broken copy
                        // action pass by leaving the expected link behind.
                        _ = try fixture("clipboard", body: "QA export pending")
                        tap(share)
                        XCTAssertTrue(wait { !self.app.buttons[share].exists })
                        let exported = try exportedLink(containing: "visual.example.test")
                        visible(row("QA Visual")).press(forDuration: 1.2)
                        tap(showQr)
                        let qrLabel = language == "English" ? "Generated QR code" : language == "简体中文" ? "生成的二维码" : "產生的二維碼"
                        let qr = app.images[qrLabel]
                        XCTAssertTrue(qr.waitForExistence(timeout: timeout))
                        assertQr(qr, equals: exported)
                        captureStable("\(language)-\(dark)-qr")
                        tap(close)
                    }

                }
            }
        }
    }

    /// UX walkthrough: captures every page and interaction state as named
    /// screenshot attachments for design review. Assertions are deliberately
    /// minimal — navigation mirrors the focused tests above, but a missing
    /// optional element is recorded as a screenshot, not a failure.
    func testUxReviewWalkthrough() throws {
        // --- Empty states (setUp landed on home; no nodes yet) ---
        captureStable("01-home-empty")
        open("profiles")
        captureStable("02-profiles-empty")
        open("rules")
        captureStable("03-rules-empty")
        open("settings")
        captureStable("04-settings")

        // --- Import flow, manual nodes ---
        open("profiles")
        tap("Add nodes or subscription")
        captureStable("05-import-empty")
        try readClipboard("not-a-node")
        tap("Preview")
        _ = app.staticTexts.matching(NSPredicate(format: "label CONTAINS 'No importable'")).firstMatch.waitForExistence(timeout: timeout)
        captureStable("06-import-invalid")
        let batch = [
            "vless://11111111-1111-1111-1111-111111111111@tokyo.example.test:443?security=tls#Tokyo%20Edge%2001",
            "trojan://secret@frankfurt.example.test:443?security=tls#Frankfurt%20Relay%2002%20with%20a%20deliberately%20long%20display%20name%20for%20truncation",
            "ss://YWVzLTI1Ni1nY206cGFzc3dvcmQ=@singapore.example.test:8388#Singapore%20Entry%2003",
        ].joined(separator: "\n")
        try readClipboard(batch)
        captureStable("07-import-filled")
        tap("Preview")
        XCTAssertTrue(app.buttons["Confirm import"].waitForExistence(timeout: timeout))
        captureStable("08-import-preview")
        tap("Confirm import")
        XCTAssertTrue(app.buttons["Choose a node"].waitForExistence(timeout: timeout))
        captureStable("09-import-choose-node")
        tap("Choose a node")
        tap("Manage nodes")
        _ = row("Tokyo Edge 01").waitForExistence(timeout: timeout)
        captureStable("10-profiles-populated")

        // --- Subscription import (brings policy groups + read-only nodes) ---
        tap("Add nodes or subscription")
        try readClipboard(required("QA_SUBSCRIPTION_URL"))
        tap("Preview")
        captureStable("11-import-subscription-preview")
        XCTAssertTrue(app.buttons["Confirm import"].waitForExistence(timeout: timeout))
        tap("Confirm import")
        if app.buttons["Choose a node"].waitForExistence(timeout: timeout) {
            captureStable("11b-subscription-choose-node")
            app.buttons["Choose a node"].tap()
            tap("Manage nodes")
        }
        _ = row("QA Subscription A").waitForExistence(timeout: timeout)
        captureStable("12-profiles-with-subscription")

        // --- Policy groups disclosure ---
        tap("Policy groups")
        captureStable("13-policy-groups")
        let group = app.buttons.matching(NSPredicate(format: "label CONTAINS 'Auto' AND NOT (label CONTAINS 'tab')")).firstMatch
        if group.exists {
            group.tap()
            captureStable("14-policy-group-selected")
        }
        tap("Policy groups")

        // --- Sort sheet ---
        let sort = app.buttons["Sort order"]
        if ready(sort) {
            sort.tap()
            capture("15-sort-menu")
            // The orders are buttons to VoiceOver, so a missing one is a
            // failure, not a step to skip; and choosing the latency order
            // must not take the screen down (Hermes once lacked `toSorted`).
            let latency = app.buttons["Lowest latency"]
            XCTAssertTrue(latency.waitForExistence(timeout: 5), "sort orders are not exposed as buttons")
            latency.tap()
            XCTAssertFalse(app.descendants(matching: .any)["screen-error-fallback"].waitForExistence(timeout: 2), "sorting by latency crashed the Nodes screen")
            XCTAssertTrue(ready(sort), "the sort sheet did not close")
            sort.tap()
            let defaultOrder = app.buttons["Default order"]
            XCTAssertTrue(defaultOrder.waitForExistence(timeout: 5))
            defaultOrder.tap()
        }

        // --- Latency test via the fixture probe ---
        _ = try fixture("delay", body: "0")
        tap("Test all")
        capture("16-latency-testing")
        _ = wait(seconds: 60) { self.row("Tokyo Edge 01").label.contains(" ms") }
        captureStable("17-latency-results")

        // --- Node actions sheet: manual node (full actions) ---
        visible(row("Tokyo Edge 01")).press(forDuration: 1.2)
        _ = app.buttons["Show QR"].waitForExistence(timeout: timeout)
        captureStable("18-node-actions")
        tap("Show QR")
        captureStable("19-node-qr")
        tap("Close")

        // --- Node actions sheet: subscription node (read-only) ---
        visible(row("QA Subscription A")).press(forDuration: 1.2)
        _ = app.buttons["Show QR"].waitForExistence(timeout: timeout)
        captureStable("20-subscription-node-actions")
        if app.buttons["Close"].exists { tap("Close") }

        // --- Home with a selection, connect attempt (tunnel fails on simulator) ---
        open("home")
        captureStable("21-home-node-selected")
        tap("Connect")
        capture("22-home-connecting")
        if app.buttons["Technical details"].waitForExistence(timeout: timeout) {
            captureStable("23-home-tun-error")
            app.buttons["Technical details"].tap()
            captureStable("24-home-technical-details")
            if app.buttons["Connect"].exists || app.buttons["Disconnect"].exists {
                (app.buttons["Connect"].exists ? app.buttons["Connect"] : app.buttons["Disconnect"]).tap()
            }
            _ = app.staticTexts["Disconnected"].waitForExistence(timeout: timeout)
        }

        // --- Activity ---
        open("connections")
        captureStable("25-activity-empty")

        // --- Rules ---
        open("rules")
        captureStable("26-rules-rule-mode")
        let rule = app.staticTexts["AI services via proxy"]
        if rule.exists {
            rule.tap()
            captureStable("27-rule-details")
        }
        open("rules")
        tap("Global")
        captureStable("28-rules-global")
        tap("Rule")

        // --- Settings pages ---
        open("subscriptions")
        captureStable("29-subscriptions")
        let subscription = row("QA Subscription")
        if ready(visible(subscription)) {
            subscription.tap()
            captureStable("30-subscription-edit")
        }
        open("general")
        captureStable("31-general")
        open("dns")
        captureStable("32-dns-collapsed")
        tap("Custom & advanced")
        captureStable("33-dns-expanded")
        let bootstrap = visible(app.textFields["Bootstrap DNS"])
        if bootstrap.exists {
            bootstrap.tap()
            bootstrap.typeText("1")
            let back = app.navigationBars.buttons.firstMatch
            if back.exists { back.tap() }
            if app.alerts.buttons["Discard changes"].waitForExistence(timeout: 5) {
                captureStable("34-dns-unsaved-alert")
                app.alerts.buttons["Discard changes"].tap()
                // The screen pop the discard triggers outlives the alert; a
                // navigation-bar snapshot taken mid-pop is already stale.
                _ = wait { self.tabButton("home").isHittable }
            } else {
                open("dns")
            }
        }
        open("maintenance")
        captureStable("35-maintenance")
        open("logs")
        captureStable("36-logs")
        open("about")
        captureStable("37-about")
        let licenses = app.buttons["Open-source licenses"]
        if ready(licenses) {
            licenses.tap()
            captureStable("38-about-licenses")
        }

        // --- Dark mode across the tabs ---
        open("general")
        tap("Dark")
        captureStable("39-dark-general")
        open("home")
        captureStable("40-dark-home")
        open("profiles")
        captureStable("41-dark-profiles")
        open("rules")
        captureStable("42-dark-rules")
        open("settings")
        captureStable("43-dark-settings")

        // --- Simplified Chinese across the tabs (back to light first) ---
        open("general")
        visible(app.buttons["Light"]).tap()
        visible(row("简体中文")).tap()
        open("home")
        captureStable("44-zh-home")
        open("profiles")
        captureStable("45-zh-profiles")
        open("rules")
        captureStable("46-zh-rules")
        open("settings")
        captureStable("47-zh-settings")
        open("general")
        captureStable("48-zh-general")
        visible(row("English")).tap()
    }

    /// Supplemental captures for pages the main walkthrough's guards skipped:
    /// rule details, subscription editing, about licenses. Each smoke run gets
    /// a fresh device, so the subscription edit step seeds its own data.
    func testUxReviewSupplement() throws {
        open("rules")
        let rule = app.buttons.matching(NSPredicate(format: "label CONTAINS 'AI services via proxy'")).firstMatch
        if ready(visible(rule)) {
            rule.tap()
            captureStable("s1-rule-details")
        }
        open("profiles")
        try importText(required("QA_SUBSCRIPTION_URL"))
        open("subscriptions")
        // An unnamed subscription is listed under its URL's host, which is
        // whatever LAN address the machine running the fixtures has.
        let subscription = row(URL(string: required("QA_SUBSCRIPTION_URL"))?.host ?? "")
        if ready(visible(subscription)) {
            subscription.tap()
            captureStable("s2-subscription-edit")
        }
        open("about")
        let licenses = app.buttons["Open-source licenses"]
        if ready(visible(licenses)) {
            licenses.tap()
            captureStable("s3-about-licenses")
        }
    }

    /// On a simulator the VPN save always fails as a declined authorization,
    /// which is exactly the state the permission guidance is for: the banner
    /// must explain the permission, and the failed connect must offer its
    /// user-initiated retry.
    func testDeclinedConnectOffersAuthorizeAgain() throws {
        open("profiles")
        try importText("vless://33333333-3333-3333-3333-333333333333@authorize.example.test:443?security=tls#Authorize%20Me")
        row("Authorize Me").tap()
        open("home")
        tap("Connect")
        let authorize = app.buttons["Authorize again"]
        XCTAssertTrue(authorize.waitForExistence(timeout: timeout), "a declined prompt must offer Authorize again")
        captureStable("declined-connect")
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS 'System authorization was not granted'")).firstMatch.exists)
    }

    func testRuleLibraryUpdate() {
        open("maintenance")
        let update = visible(app.buttons.matching(NSPredicate(format: "label CONTAINS 'Update now'")).firstMatch)
        XCTAssertTrue(ready(update))
        update.tap()
        XCTAssertTrue(wait(seconds: 300) {
            self.app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH 'Updated ' AND label CONTAINS 'files'")).firstMatch.exists
        }, "a completed download must show its updated resource count")
    }

    private func assertQr(_ qr: XCUIElement, equals expected: String) {
        // Large text puts identity details above the QR in a scrollable sheet.
        // Reveal the entire image before decoding, not just its tappable edge.
        for _ in 0..<8 {
            let overflow = qr.frame.maxY - (app.frame.maxY - 48)
            if overflow <= 0 { break }
            let distance = min(overflow + 8, app.frame.height * 0.3) / app.frame.height
            let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.8))
            let end = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.8 - distance))
            start.press(forDuration: 0.1, thenDragTo: end)
        }
        // Decode the rendered native image; accessibility labels alone do not
        // prove the exported link is scannable or belongs to the selected node.
        let request = VNDetectBarcodesRequest()
#if targetEnvironment(simulator)
        // Revision 4 cannot create its inference context in this simulator.
        // A device takes the newest revision its system has.
        request.revision = VNDetectBarcodesRequestRevision1
#endif
        request.symbologies = [.qr]
        var failure: String?
        XCTAssertTrue(wait {
            guard let image = qr.screenshot().image.cgImage else { return false }
            do { try VNImageRequestHandler(cgImage: image).perform([request]) }
            catch { failure = String(describing: error); return false }
            return (request.results ?? []).contains { $0.payloadStringValue == expected.trimmingCharacters(in: .whitespacesAndNewlines) }
        }, failure ?? "Rendered QR must decode to the complete exported share link")
    }

    private func required(_ key: String) -> String {
        let value = ProcessInfo.processInfo.environment[key] ?? ""
        XCTAssertFalse(value.isEmpty, "Run vp run check mobile ios smoke; missing \(key)")
        return value
    }

    private func fixture(_ path: String, body: String? = nil) throws -> Data {
        let url = URL(string: required("QA_CONTROL_URL") + "/" + path)!
        var request = URLRequest(url: url)
        if let body { request.httpMethod = "POST"; request.httpBody = Data(body.utf8) }
        let done = expectation(description: "fixture \(path)")
        var result: Result<Data, Error>?
        URLSession.shared.dataTask(with: request) { data, _, error in
            result = error.map { .failure($0) } ?? .success(data ?? Data())
            done.fulfill()
        }.resume()
        wait(for: [done], timeout: 15)
        return try XCTUnwrap(result).get()
    }

    /// Puts `value` on the simulator clipboard and reads it into the import field.
    private func readClipboard(_ value: String) throws {
        _ = try fixture("clipboard", body: value)
        XCTAssertEqual(String(data: try fixture("clipboard"), encoding: .utf8), value, "Simulator clipboard fixture changed before import")
        if !app.buttons["Read clipboard"].exists { tap("Add nodes or subscription") }
        tap("Read clipboard")
        let allow = XCUIApplication(bundleIdentifier: "com.apple.springboard").buttons["Allow Paste"]
        if allow.waitForExistence(timeout: 1) { allow.tap() }
    }

    private func importText(_ value: String) throws {
        try readClipboard(value)
        if value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return }
        tap("Preview")
        if value == "not-a-node" { return }
        XCTAssertTrue(app.buttons["Confirm import"].waitForExistence(timeout: timeout))
        tap("Confirm import")
        XCTAssertTrue(wait {
            self.app.buttons["Choose a node"].exists || self.app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Connect to '")).firstMatch.exists
        })
        // Import never connects by itself. Management tests continue at the full list.
        open("profiles")
    }

    private func exportedLink(containing host: String) throws -> String {
        // Closing the sheet and RN's void setString() can precede the system
        // pasteboard write. Wait for the actual value, not a closing animation
        // or a success label; each read is an asynchronous fixture round trip.
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let value = String(data: try fixture("clipboard"), encoding: .utf8) ?? ""
            if value.contains(host) { return value }
            // Each read makes the fixture server spawn `simctl pbpaste`.
            Thread.sleep(forTimeInterval: 0.25)
        }
        XCTFail("The exported share link must reach the system clipboard")
        return ""
    }

    private func row(_ name: String) -> XCUIElement {
        app.buttons.matching(NSPredicate(format: "label BEGINSWITH[c] %@", name)).firstMatch
    }

    /// The system owns the tab buttons: identify them inside its bar by the
    /// app's translated titles, including while the visual matrix changes locale.
    private func tabButton(_ page: String) -> XCUIElement {
        let labels = [
            "home": ["Home", "主页", "主頁"],
            "profiles": ["Nodes", "节点", "節點"],
            "rules": ["Rules", "规则", "規則"],
            "settings": ["Settings", "设置", "設定"],
        ]
        // iPadOS 26 exposes its top tab strip as ordinary nested buttons,
        // without a TabBar ancestor. Their titles remain the same native labels.
        let buttons = UIDevice.current.userInterfaceIdiom == .pad ? app.buttons : app.tabBars.buttons
        return buttons.matching(NSPredicate(format: "label IN %@", labels[page] ?? [page])).firstMatch
    }

    private func open(_ page: String) {
        for _ in 0..<5 {
            if tabButton("home").isHittable { break }
            let back = app.navigationBars.buttons.firstMatch
            if back.exists { back.tap() }
            if app.alerts.buttons["Discard changes"].exists { app.alerts.buttons["Discard changes"].tap() }
        }
        let settingsPages = ["subscriptions", "general", "dns", "maintenance", "about"]
        let target = settingsPages.contains(page) || page == "logs" ? "settings" : page == "connections" ? "home" : page
        let tab = tabButton(target)
        XCTAssertTrue(tab.exists || tab.waitForExistence(timeout: timeout))
        XCTAssertTrue(ready(tab))
        tab.tap()
        XCTAssertTrue(wait { tab.isSelected })
        if settingsPages.contains(page) { visible(app.buttons["settings-" + page]).tap() }
        if page == "connections" { visible(tabButton("settings")).tap(); visible(app.buttons["settings-maintenance"]).tap(); visible(app.buttons["maintenance-activity"]).tap() }
        if page == "logs" { visible(app.buttons["settings-maintenance"]).tap(); visible(app.buttons["maintenance-logs"]).tap() }
    }

    private func tap(_ label: String) {
        let button = app.buttons[label]
        XCTAssertTrue(button.waitForExistence(timeout: timeout), label)
        if !button.isHittable { _ = visible(button) }
        XCTAssertTrue(ready(button), label)
        button.tap()
    }

    private func relaunch() { app.terminate(); launch() }

    /// Launches the app and waits for the tabs. On a simulator's first launch
    /// the data notice stands in front of everything else (App Store Guideline
    /// 5.4); it is captured as evidence and accepted. The acceptance persists,
    /// so later launches go straight to the tabs.
    private func launch() {
        app.launch()
        let accept = app.buttons["privacy-continue"]
        let tabs = tabButton("home")
        XCTAssertTrue(wait(seconds: timeout) { accept.exists || tabs.exists })
        if accept.exists {
            capture("privacy-notice")
            visible(accept).tap()
        }
        XCTAssertTrue(tabs.waitForExistence(timeout: timeout))
    }

    private func visible(_ element: XCUIElement) -> XCUIElement {
        for _ in 0..<8 {
            if element.exists && element.isHittable { break }
            app.swipeUp()
        }
        if !element.exists || !element.isHittable {
            for _ in 0..<12 {
                if element.exists && element.isHittable { break }
                app.swipeDown()
            }
        }
        return element
    }

    private func ready(_ element: XCUIElement) -> Bool { wait { element.exists && element.isEnabled && element.isHittable } }

    private func wait(seconds: TimeInterval = 30, _ condition: @escaping () -> Bool) -> Bool {
        if condition() { return true }
        let predicate = NSPredicate { _, _ in condition() }
        return XCTWaiter.wait(for: [XCTNSPredicateExpectation(predicate: predicate, object: nil)], timeout: seconds) == .completed
    }

    private func captureStable(_ title: String) {
        // Wait for unchanged rendered pixels, not a guessed animation delay.
        // Closing sheets may keep painting after their hit regions are gone.
        var previous: Data?
        var screenshot = app.screenshot()
        XCTAssertTrue(wait(seconds: 15) {
            screenshot = self.app.screenshot()
            guard let current = screenshot.image.cgImage?.dataProvider?.data as Data? else { return false }
            let settled = previous == current
            previous = current
            return settled
        }, "The page should reach a stable rendered state")
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = title
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    private func capture(_ title: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = title
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
