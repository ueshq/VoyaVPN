import XCTest
import Vision

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
        open("settings")
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
        for page in ["home", "profiles", "rules", "connections", "settings"] {
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
        XCTAssertTrue(app.staticTexts["Clipboard is empty."].waitForExistence(timeout: timeout))
        try importText("not-a-node")
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS 'no importable'")).firstMatch.waitForExistence(timeout: timeout))
        let link = "vless://22222222-2222-2222-2222-222222222222@qa.example.test:443?security=tls#QA%20Lifecycle"
        try importText(link)
        XCTAssertTrue(row(title).waitForExistence(timeout: timeout))
        try importText(link)
        XCTAssertEqual(app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", title)).count, 1)
        row(title).press(forDuration: 1.2)
        _ = try fixture("clipboard", body: "QA export pending")
        tap("Share links")
        XCTAssertTrue(wait { !self.app.buttons["Share links"].exists })
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
        XCTAssertTrue(row(title).label.contains("Selected"))
        open("home")
        tap("Connect")
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH 'VPN unsupported:' OR label BEGINSWITH 'Could not connect:'")).firstMatch.waitForExistence(timeout: timeout))
        tap("Connect")
        XCTAssertTrue(app.staticTexts["Disconnected"].exists)
        open("profiles")
        row(title).press(forDuration: 1.2)
        tap("Delete")
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
        let group = app.buttons.matching(NSPredicate(format: "label CONTAINS 'Auto' AND NOT (label CONTAINS 'tab')")).firstMatch
        XCTAssertTrue(group.waitForExistence(timeout: timeout))
        group.tap()
        XCTAssertTrue(wait { group.isSelected })
        relaunch()
        open("profiles")
        XCTAssertTrue(group.isSelected)
        let update = app.buttons.matching(NSPredicate(format: "label BEGINSWITH 'Update subscription '")).firstMatch
        XCTAssertTrue(ready(update))
        update.tap()
        XCTAssertTrue(row("QA Subscription Updated").waitForExistence(timeout: timeout))
        tap("Update all subscriptions")
        XCTAssertTrue(row("QA Subscription Refreshed").waitForExistence(timeout: timeout))
        visible(row("QA Subscription Refreshed")).press(forDuration: 1.2)
        XCTAssertTrue(app.buttons["Share links"].waitForExistence(timeout: timeout))
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
        open("settings")
        for label in ["Check the exit IP after connecting", "Record detailed connection log", "FakeIP", "Block HTTPS/SVCB"] {
            let control = visible(app.switches[label])
            let before = control.value as? String
            control.tap()
            XCTAssertTrue(wait { control.value as? String != before })
            let expected = control.value as? String
            relaunch()
            open("settings")
            XCTAssertEqual(visible(app.switches[label]).value as? String, expected)
        }
        tap("Dark")
        relaunch()
        open("settings")
        XCTAssertTrue(app.buttons["Dark"].isSelected)
    }

    func testDnsValidationAndPersistence() {
        open("settings")
        for label in ["Remote DNS", "Direct DNS", "Bootstrap DNS"] {
            let field = visible(app.textFields[label])
            XCTAssertTrue(field.exists)
            let original = field.value as? String ?? ""
            let prefix = original.hasPrefix("https://") ? "https://" : String(original.prefix(1))
            let tail = prefix == "https://" ? "1.1.1.1/dns-query" : ".1.1.1"
            field.coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap()
            let count = max(0, original.count - prefix.count)
            field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: count) + tail + "\n")
            open("home")
            relaunch()
            open("settings")
            XCTAssertEqual(visible(app.textFields[label]).value as? String, prefix + tail)
        }
        let bootstrap = visible(app.textFields["Bootstrap DNS"])
        bootstrap.coordinate(withNormalizedOffset: CGVector(dx: 0.95, dy: 0.5)).tap()
        bootstrap.typeText(":99999")
        XCTAssertTrue(app.staticTexts.matching(NSPredicate(format: "label CONTAINS '65535' OR label CONTAINS '65,535' OR label CONTAINS '99999'")).firstMatch.waitForExistence(timeout: timeout))
        relaunch()
        open("settings")
        XCTAssertFalse((visible(app.textFields["Bootstrap DNS"]).value as? String ?? "").contains(":99999"))
    }

    func testVisualMatrix() throws {
        open("profiles")
        try importText("vless://77777777-7777-7777-7777-777777777777@visual.example.test:443?security=tls#QA%20Visual")
        for (language, _) in [("English", "en"), ("简体中文", "zh-Hans"), ("繁體中文", "zh-Hant")] {
            open("settings")
            visible(row(language)).tap()
            for dark in [false, true] {
                open("settings")
                let label = language == "English" ? (dark ? "Dark" : "Light") : language == "简体中文" ? (dark ? "深色" : "浅色") : (dark ? "深色" : "淺色")
                visible(app.buttons[label]).tap()
                for page in ["home", "profiles", "rules", "connections", "settings"] {
                    open(page)
                    captureStable("\(language)-\(dark ? "dark" : "light")-\(page)")
                    if page == "profiles" {
                        visible(row("QA Visual")).press(forDuration: 1.2)
                        let showQr = language == "English" ? "Show QR" : language == "简体中文" ? "显示二维码" : "顯示 QR Code"
                        let close = language == "English" ? "Close" : language == "简体中文" ? "关闭" : "關閉"
                        XCTAssertTrue(ready(app.buttons[showQr]))
                        captureStable("\(language)-\(dark)-actions")
                        let share = language == "English" ? "Share links" : language == "简体中文" ? "分享链接" : "分享連結"
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
        open("settings")
        let update = visible(app.buttons.matching(NSPredicate(format: "label CONTAINS 'Update now'")).firstMatch)
        XCTAssertTrue(ready(update))
        update.tap()
        XCTAssertTrue(wait(seconds: 300) {
            self.app.staticTexts.matching(NSPredicate(format: "label BEGINSWITH 'Updated ' AND label CONTAINS 'files'")).firstMatch.exists
        }, "a completed download must show its updated resource count")
    }

    private func assertQr(_ qr: XCUIElement, equals expected: String) {
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
        tap("Import from clipboard")
        // The paste prompt blocks the target app's main thread. Query SpringBoard
        // first; asking the blocked app for its hierarchy deadlocks automation.
        for host in [XCUIApplication(bundleIdentifier: "com.apple.springboard")] {
            let allow = host.buttons["Allow Paste"]
            if allow.waitForExistence(timeout: 1) { allow.tap() }
        }
        XCTAssertTrue(ready(app.buttons["Import from clipboard"]))
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
        let tab = app.buttons["tab-" + page]
        XCTAssertTrue(tab.exists || tab.waitForExistence(timeout: timeout))
        XCTAssertTrue(ready(tab))
        tab.tap()
        XCTAssertTrue(wait { tab.isSelected })
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
