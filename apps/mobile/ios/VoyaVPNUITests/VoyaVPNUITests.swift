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
        app.launch()
        XCTAssertTrue(app.buttons["tab-settings"].waitForExistence(timeout: timeout))
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
            XCTAssertTrue(wait { self.app.buttons["tab-" + page].isSelected })
        }
        open("connections")
        XCTAssertTrue(app.staticTexts["Connect to view network activity"].exists)
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
        XCTAssertTrue(app.buttons["Technical details"].waitForExistence(timeout: timeout))
        tap("Connect")
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
        tap("Manage subscriptions")
        tap("Update all subscriptions")
        open("profiles")
        XCTAssertTrue(row("QA Subscription Updated").waitForExistence(timeout: timeout))
        tap("Manage subscriptions")
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
        open("general")
        for label in ["Check the exit IP after connecting", "Record detailed connection log"] {
            let control = visible(app.switches[label])
            let before = control.value as? String
            control.tap()
            XCTAssertTrue(wait { control.value as? String != before })
            let expected = control.value as? String
            relaunch()
            open("general")
            XCTAssertEqual(visible(app.switches[label]).value as? String, expected)
        }
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

    func testSecondaryPagesAndImportCancellation() {
        for page in ["subscriptions", "connections", "dns", "maintenance", "logs", "about"] {
            open(page)
            captureStable(page)
            XCTAssertFalse(app.buttons["tab-home"].isHittable)
        }
        open("profiles")
        tap("Add nodes or subscription")
        let input = app.textViews["Add nodes or subscription"]
        XCTAssertTrue(input.waitForExistence(timeout: timeout))
        input.tap()
        input.typeText("trojan://test@cancel.example.test:443#Cancelled")
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
                XCTAssertTrue(app.buttons["tab-" + page].isHittable)
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
        // Revision 4 cannot create its inference context in this simulator.
        // Revision 1 decodes the same rendered image on simulator and device.
        request.revision = VNDetectBarcodesRequestRevision1
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
        XCTAssertFalse(value.isEmpty, "Run pnpm check:mobile:ios:smoke; missing \(key)")
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

    private func importText(_ value: String) throws {
        _ = try fixture("clipboard", body: value)
        XCTAssertEqual(String(data: try fixture("clipboard"), encoding: .utf8), value, "Simulator clipboard fixture changed before import")
        if !app.buttons["Read clipboard"].exists { tap("Add nodes or subscription") }
        tap("Read clipboard")
        for host in [XCUIApplication(bundleIdentifier: "com.apple.springboard")] {
            let allow = host.buttons["Allow Paste"]
            if allow.waitForExistence(timeout: 1) { allow.tap() }
        }
        if value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return }
        tap("Preview")
        if value == "not-a-node" { return }
        XCTAssertTrue(app.buttons["Confirm import"].waitForExistence(timeout: timeout))
        tap("Confirm import")
        XCTAssertTrue(app.buttons["Choose a node"].waitForExistence(timeout: timeout))
        tap("Choose a node")
    }

    private func exportedLink(containing host: String) throws -> String {
        // Closing the sheet and RN's void setString() can precede the system
        // pasteboard write. Wait for the actual value, not a closing animation
        // or a success label; each read is an asynchronous fixture round trip.
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let value = String(data: try fixture("clipboard"), encoding: .utf8) ?? ""
            if value.contains(host) { return value }
        }
        XCTFail("The exported share link must reach the system clipboard")
        return ""
    }

    private func row(_ name: String) -> XCUIElement {
        app.buttons.matching(NSPredicate(format: "label BEGINSWITH[c] %@", name)).firstMatch
    }

    private func open(_ page: String) {
        for _ in 0..<5 {
            if app.buttons["tab-home"].isHittable { break }
            let back = app.navigationBars.buttons.firstMatch
            if back.exists { back.tap() }
            if app.alerts.buttons["Discard changes"].exists { app.alerts.buttons["Discard changes"].tap() }
        }
        let settingsPages = ["general", "dns", "maintenance", "about"]
        let target = settingsPages.contains(page) || page == "logs" ? "settings" : page == "connections" ? "home" : page == "subscriptions" ? "profiles" : page
        let tab = app.buttons["tab-" + target]
        XCTAssertTrue(tab.exists || tab.waitForExistence(timeout: timeout))
        XCTAssertTrue(ready(tab))
        tab.tap()
        XCTAssertTrue(wait { tab.isSelected })
        if settingsPages.contains(page) { visible(app.buttons["settings-" + page]).tap() }
        if page == "connections" { visible(app.buttons["home-activity"]).tap() }
        if page == "subscriptions" { tap("Manage subscriptions") }
        if page == "logs" { visible(app.buttons["settings-maintenance"]).tap(); visible(app.buttons["maintenance-logs"]).tap() }
    }

    private func tap(_ label: String) {
        let button = app.buttons[label]
        XCTAssertTrue(button.waitForExistence(timeout: timeout), label)
        if !button.isHittable { _ = visible(button) }
        XCTAssertTrue(ready(button), label)
        button.tap()
    }

    private func relaunch() { app.terminate(); app.launch(); XCTAssertTrue(app.buttons["tab-home"].waitForExistence(timeout: timeout)) }

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
