import XCTest

/// The simulator half of the acceptance checks.
///
/// What these prove is the one thing the Jest suite cannot: that the screens
/// are driving the **real** backend — `crates/voya-mobile-ffi` through the
/// `VoyaNative` TurboModule — rather than the shared in-memory mock that
/// `transport.ts` falls back to when the module is missing. Every case below
/// is a round trip into Rust and back.
///
/// They run in order and share one install, because the app's database is the
/// state under test: a node imported by one case is what the next selects,
/// shares and finally deletes. XCTest runs a class's tests alphabetically,
/// which is what the numeric prefixes are for. The runner uninstalls the app
/// first so the database starts empty — see `docs/release/mobile-ios-signing.md`.
///
/// Not here, because a simulator cannot do them: starting a tunnel, the VPN
/// authorization prompt, and anything that needs the extension to run. Those
/// stay in the device checklist.
final class VoyaVPNUITests: XCTestCase {
    private var app: XCUIApplication!

    /// Generous: a cold React Native launch plus opening SQLite is not fast,
    /// and a flaky timeout reads as a product failure.
    private let timeout: TimeInterval = 30

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
    }

    // MARK: - Cases

    /// The one that decides whether any of the rest means anything.
    ///
    /// The mock backend seeds three nodes (🇯🇵 Tokyo, 🇸🇬 Singapore,
    /// 🇺🇸 Los Angeles). The real one opens an empty database. Seeing Tokyo
    /// here means `TurboModuleRegistry.get("VoyaNative")` returned null and the
    /// app quietly fell back to the mock.
    func test00LaunchesOnTheRealBackend() {
        launch()
        open(tab: "Nodes")

        XCTAssertTrue(waitFor(app.staticTexts["No nodes"]), "the node list should start empty")
        XCTAssertFalse(app.staticTexts["🇯🇵 Tokyo"].exists, "this is the mock backend, not the host")
    }

    func test01ImportsANodeFromTheClipboard() {
        UIPasteboard.general.string = Self.shareLink
        launch()
        open(tab: "Nodes")

        app.buttons["Import from clipboard"].tap()
        allowPasteIfAsked()

        XCTAssertTrue(waitFor(nodeRow()), "the imported node should be listed")
    }

    /// Selecting a node saves it *and* asks to connect. The simulator has no
    /// tunnel, so the connect half fails — both halves are real round trips,
    /// and the save is the one that has to stick.
    func test02SelectsTheImportedNode() {
        launch()
        open(tab: "Nodes")

        let node = nodeRow()
        XCTAssertTrue(waitFor(node))
        node.tap()

        // The row's label carries its state, so this is the same element
        // re-read rather than a separate badge.
        XCTAssertTrue(
            waitFor(app.buttons.matching(
                NSPredicate(format: "label BEGINSWITH %@ AND label CONTAINS 'Selected'", Self.nodeName)
            ).firstMatch),
            "the backend should have recorded the active node"
        )
    }

    func test03SwitchesTrafficMode() {
        launch()
        open(tab: "Rules")

        // Both start disabled until the app has read the core's state back;
        // tapping before then is a no-op that reads as a product failure.
        let global = app.buttons["Global"]
        XCTAssertTrue(waitForEnabled(global), "the mode switcher should come out of its pending state")
        global.tap()

        let banner = app.staticTexts[
            "Global mode is on: all captured traffic goes through the proxy and these rules are skipped."
        ]
        XCTAssertTrue(waitFor(banner), "global mode should say the rules are skipped")

        XCTAssertTrue(waitForEnabled(app.buttons["Rule"]))
        app.buttons["Rule"].tap()
        XCTAssertTrue(waitForDisappearance(banner), "leaving global mode should clear the banner")
    }

    /// The rules a fresh install seeds are the backend's own defaults, so this
    /// also checks they arrive named rather than as their reserved remarks.
    func test04TogglesASeededRule() {
        launch()
        open(tab: "Rules")

        let rule = app.switches["AI services via proxy"]
        XCTAssertTrue(waitFor(rule), "the default rule set should be seeded and named")

        let before = rule.value as? String
        rule.tap()

        open(tab: "Home")
        open(tab: "Rules")
        XCTAssertTrue(waitFor(app.switches["AI services via proxy"]))
        XCTAssertNotEqual(
            app.switches["AI services via proxy"].value as? String,
            before,
            "the new state should have survived leaving the screen"
        )
    }

    func test05RunsALatencyTest() {
        launch()
        open(tab: "Nodes")

        let run = app.buttons["Test all"]
        XCTAssertTrue(waitForEnabled(run), "a listed node should make the run available")
        run.tap()

        // The node points at a hostname that does not resolve, so the outcome
        // is a failure — but the row moving off "Not tested" at all means the
        // run reached the probe core and came back per node. Asserting on the
        // button instead would prove nothing: it is titled "Test all" again the
        // moment the run ends, and it was titled that before it started.
        XCTAssertTrue(
            waitFor(
                app.buttons.matching(NSPredicate(
                    format: "label BEGINSWITH %@ AND NOT (label CONTAINS 'Not tested')",
                    Self.nodeName
                )).firstMatch,
                seconds: 120
            ),
            "every node should come back with an outcome"
        )
    }

    func test06ActivityAsksForAConnectionFirst() {
        launch()
        open(tab: "Network activity")

        XCTAssertTrue(waitFor(app.staticTexts["Connect to view network activity"]))
    }

    func test07SavesADnsResolver() {
        launch()
        open(tab: "Settings")

        let field = app.textFields["Remote DNS"]
        XCTAssertTrue(waitFor(field))
        XCTAssertEqual(
            field.value as? String,
            Self.seededResolver,
            "a fresh install should carry the backend's own default"
        )

        tapPastEndOfText(field)
        deleteDown(to: Self.resolverScheme.count, in: field)
        type(Self.resolverTail, into: field)

        // The draft debounces and writes on its own; leaving and coming back is
        // what proves the value went to the backend rather than staying in React.
        open(tab: "Home")
        open(tab: "Settings")
        let reloaded = app.textFields["Remote DNS"]
        XCTAssertTrue(waitFor(reloaded))
        XCTAssertEqual(reloaded.value as? String, Self.resolver)
    }

    func test08SwitchesTheme() {
        launch()
        open(tab: "Settings")

        let dark = app.buttons["Dark"]
        XCTAssertTrue(waitFor(dark))
        dark.tap()

        XCTAssertTrue(
            waitFor(app.buttons["Follow system"]),
            "the other choices stay on screen"
        )
        XCTAssertTrue(dark.isSelected, "the chosen theme should be marked")
    }

    /// The rule library is fetched over the network, so how long it takes is
    /// not this test's business — on this machine the same download has taken
    /// under a minute and over three. What is asserted is the round trip: the
    /// press reached the backend, and the backend came back.
    func test09RefreshesTheRuleLibrary() {
        launch()
        open(tab: "Settings")

        let never = app.staticTexts["Not updated from this device yet"]
        XCTAssertTrue(waitFor(never), "a fresh install has never updated the rule library")

        // Matched loosely because the spinner prepends "In progress" to the
        // label for the length of the run — an exact-label query loses the
        // element at exactly the moment this test is watching it.
        let update = app.buttons
            .matching(NSPredicate(format: "label CONTAINS 'Update now'")).firstMatch
        XCTAssertTrue(waitFor(scrollTo(update)))
        update.tap()

        // Losing the button to its disabled state is the proof the command was
        // dispatched rather than swallowed...
        XCTAssertTrue(
            wait(until: { !update.isEnabled || !never.exists }, seconds: 30),
            "pressing should put the card into its updating state"
        )
        // ...and getting it back is the proof the backend answered. A failed
        // download hands it back just as surely as a successful one; only
        // silence would leave it disabled forever. A success additionally
        // replaces the "never" line with a timestamp and a file count, which is
        // what normally happens here.
        XCTAssertTrue(
            wait(until: { update.exists && update.isEnabled }, seconds: 300),
            "the rule library should report an outcome"
        )
    }

    func test10ShowsAShareQrCode() {
        launch()
        open(tab: "Nodes")

        let node = nodeRow()
        XCTAssertTrue(waitFor(node))
        node.press(forDuration: 1.2)

        XCTAssertTrue(waitFor(app.buttons["Show QR"]), "a long press should open the actions sheet")
        app.buttons["Show QR"].tap()

        // Rendered by `generate_qr_code` in Rust and drawn by react-native-svg;
        // neither half is exercised anywhere else.
        XCTAssertTrue(waitFor(app.images["Generated QR code"], seconds: 45))
    }

    func test11KeepsTheNodeAcrossARelaunch() {
        launch()
        open(tab: "Nodes")
        XCTAssertTrue(waitFor(nodeRow()))

        app.terminate()
        launch()
        open(tab: "Nodes")

        XCTAssertTrue(
            waitFor(nodeRow()),
            "the node should have been written to the app's own SQLite database"
        )
    }

    func test12DeletesTheNode() {
        launch()
        open(tab: "Nodes")

        let node = nodeRow()
        XCTAssertTrue(waitFor(node))
        node.press(forDuration: 1.2)

        XCTAssertTrue(waitFor(app.buttons["Delete"]))
        app.buttons["Delete"].tap()

        XCTAssertTrue(waitFor(app.staticTexts["No nodes"]), "the list should be empty again")
    }

    // MARK: - Fixtures

    private static let nodeName = "Simulator node"
    private static let seededResolver = "https://cloudflare-dns.com/dns-query"
    /// The edit keeps the scheme of the value it replaces, and the test deletes
    /// down to it rather than clearing the field: an empty resolver means "use
    /// the default" to the backend, which hands the default straight back, so
    /// the field has to go from one non-empty value to another.
    private static let resolverScheme = "https://"
    private static let resolverTail = "9.9.9.9/dns-query"
    private static let resolver = resolverScheme + resolverTail
    private static let shareLink =
        "vless://11111111-1111-1111-1111-111111111111@node.example.test:443"
            + "?security=tls&sni=node.example.test&type=ws&path=%2Fws#Simulator%20node"

    // MARK: - Helpers

    private func launch() {
        app.launch()
        // The shell suspends on the locale bundle, so the first frame is blank.
        XCTAssertTrue(waitFor(tab("Home")), "the tab bar should come up")
    }

    private func open(tab name: String) {
        let button = tab(name)
        XCTAssertTrue(waitFor(button), "the \(name) tab should exist")
        button.tap()
    }

    /// React Navigation spells a tab's accessibility label out in full —
    /// `Home, tab, 1 of 5` — so an exact-label query never matches. Matching the
    /// prefix keeps the queries readable without pinning the tab count.
    private func tab(_ name: String) -> XCUIElement {
        app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", "\(name), tab")).firstMatch
    }

    @discardableResult
    private func waitFor(_ element: XCUIElement, seconds: TimeInterval? = nil) -> Bool {
        element.waitForExistence(timeout: seconds ?? timeout)
    }

    private func waitForDisappearance(_ element: XCUIElement, seconds: TimeInterval? = nil) -> Bool {
        let gone = expectation(for: NSPredicate(format: "exists == false"), evaluatedWith: element)
        return XCTWaiter().wait(for: [gone], timeout: seconds ?? timeout) == .completed
    }

    /// Existing is not the same as usable: the mode switcher and the speedtest
    /// button both render disabled until a backend read lands.
    private func waitForEnabled(_ element: XCUIElement, seconds: TimeInterval? = nil) -> Bool {
        let ready = expectation(
            for: NSPredicate(format: "exists == true AND isEnabled == true"),
            evaluatedWith: element
        )
        return XCTWaiter().wait(for: [ready], timeout: seconds ?? timeout) == .completed
    }

    /// The imported node's row.
    ///
    /// One `Button`, not the three texts it draws: React Native's `Pressable`
    /// is an accessibility element itself, so the name, the address and the
    /// latest latency arrive folded into a single label.
    private func nodeRow() -> XCUIElement {
        app.buttons.matching(NSPredicate(format: "label BEGINSWITH %@", Self.nodeName)).firstMatch
    }

    private func scrollTo(_ element: XCUIElement) -> XCUIElement {
        var attempts = 0
        while !element.exists && attempts < 6 {
            app.swipeUp()
            attempts += 1
        }
        return element
    }

    /// Polls a condition that no single element's state can express.
    private func wait(until condition: () -> Bool, seconds: TimeInterval) -> Bool {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline {
            if condition() { return true }
            Thread.sleep(forTimeInterval: 0.5)
        }
        return condition()
    }

    /// Focuses a text field with the caret after its last character.
    ///
    /// `tap()` hits the field's centre, which drops the caret wherever that
    /// lands *inside* the text: deleting `value.count` characters from there
    /// ate the middle of `https://cloudflare-dns.com/dns-query` and left
    /// `com/dns-query` behind. Tapping past the end of the text puts the caret
    /// at the end, and it has to be the *first* tap — this field sits low
    /// enough that the keyboard covers it once it is focused, so a second tap
    /// lands on the keyboard and takes the focus away again.
    private func tapPastEndOfText(_ field: XCUIElement) {
        field.coordinate(withNormalizedOffset: CGVector(dx: 0.98, dy: 0.5)).tap()
    }

    /// Backspaces a focused field down to its first `length` characters.
    private func deleteDown(to length: Int, in field: XCUIElement) {
        for _ in 0 ..< 200 {
            guard let existing = field.value as? String, existing.count > length else { return }
            field.typeText(XCUIKeyboardKey.delete.rawValue)
        }
    }

    /// Types into a focused field, one character at a time.
    ///
    /// React Native's `TextInput` is controlled, so every keystroke round-trips
    /// through JavaScript before the field shows the result, and a single
    /// `typeText` of a whole string outruns that — a `typeText` of 36
    /// backspaces deleted 13 characters on one run and none at all on the next.
    private func type(_ text: String, into field: XCUIElement) {
        for character in text {
            field.typeText(String(character))
        }
    }

    /// iOS 16 and later asks before letting an app read a pasteboard another
    /// app filled, which is exactly what the test runner did.
    private func allowPasteIfAsked() {
        let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        let allow = springboard.buttons["Allow Paste"]
        if allow.waitForExistence(timeout: 5) {
            allow.tap()
        }
    }
}
