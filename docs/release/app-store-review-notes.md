# App Store Review Notes

This page holds the answers App Review asks for when VoyaVPN is submitted to
the Mac App Store, and later the iOS App Store. Each block below is written to
be pasted as-is into App Store Connect. Keep them true: update this page in the
same change that alters what the app listens on, connects to, or stores.

Where each block goes:

- **App Review Information → Notes** (App Store Connect, the version page):
  [the notes block](#app-review-information--notes) for the platform being
  submitted, every submission. The field holds 4,000 characters, so each block
  summarizes the two long answers below instead of repeating them.
- **Pricing and Availability**: leave out China mainland on both app records;
  see [Sales and territories](#sales-and-territories).
- **In the app**: the iOS app shows the same data facts on a "Before you
  start" screen before anything else, and again under Settings > About & help
  (Guideline 5.4).
- **Resolution Center reply**: [the reply for build 405](#resolution-center-reply-for-build-405)
  for the 2026-09 rejection; the full [entitlement explanation](#why-the-app-needs-network-server)
  or [VPN answers](#vpn-questions) as separate messages when a reviewer asks.
- **App Privacy**: "Data Not Collected". A VPN app also needs a **Privacy
  Policy URL** on the app record; that page has to state the same facts as the
  VPN answers below.
- **Privacy Policy URL**: `https://voyavpn.wangc.ai/privacy`. **Support URL**:
  `https://voyavpn.wangc.ai/support`. Both are served from `apps/web`; see
  [marketing-site.md](marketing-site.md). Its tests fail if the privacy page
  stops naming a host the VPN answers name, so change them together.

The 2026-09 Mac submission, build 405 from commit `d311a25`, was stopped for
three reasons. The build fix for the second one is in
[macos-app-store.md](macos-app-store.md); the first and third are answered
here. The resubmission order is in
[macos-app-store.md](macos-app-store.md#resubmitting-after-a-rejection).

| Review message | Answer |
| --- | --- |
| `com.apple.security.network.server` without matching functionality (2.1.0, 2.4.5, 2.5.1) | Keep the entitlement; reply with [the entitlement explanation](#why-the-app-needs-network-server). |
| Non-public API `__kCFBundleNumericVersionKey` (2.5.1) | Fixed in the binary: the bundled sing-box is built from source without the Cronet-based naive outbound. `native macos pkg` now refuses any Mach-O that imports it. |
| VPN information request | Reply with [the VPN answers](#vpn-questions). |
| Not raised yet, fixed before resubmitting (2.4.5) | macOS has no privilege-escalation path: the admin-prompt installer is compiled for Linux only, and `native macos pkg` refuses escalation text. Launch at login registers a bundled agent with `SMAppService` instead of writing `~/Library/LaunchAgents`. |

## Why the app needs network.server

Paste this into the reply and into App Review Information → Notes.

```text
VoyaVPN uses com.apple.security.network.server for three features that accept
incoming connections:

1. Self-hosted node. In the "Self-hosted node" section the user can turn this Mac into
   a proxy server for their own other devices. VoyaVPN then runs sing-box
   (bundled, sandboxed, inheriting the app's sandbox) listening on the
   VLESS/Shadowsocks ports the user chooses, on all interfaces, and accepts
   connections from those devices. This is a real inbound server.

2. Latency test while disconnected. To measure a proxy server, the app starts
   the bundled sing-box helper with a local proxy listener on 127.0.0.1 and
   sends a test request through it. The helper runs in the app's sandbox via
   com.apple.security.inherit, so the listener needs this entitlement.

3. The Packet Tunnel extension. The extension's sing-box opens a local proxy
   port and a local control API on 127.0.0.1. The app reads the control API to
   show active connections and to measure servers while connected.

Without the entitlement, each of these fails with "bind: operation not
permitted". To see (1): open "Self-hosted node", turn the node on, and connect
to the shown address from another device on the same network. To see (2): add
any server and choose "Ping" (or "Test all") while disconnected.
```

## VPN questions

Paste this into the reply and into App Review Information → Notes.

```text
What user information is the app collecting using VPN?
None. VoyaVPN is a client for proxy servers the user configures (by
subscription URL, share link, or QR code). It contains no analytics, crash
reporting, advertising, or telemetry SDK, and it has no user accounts. The VPN
tunnel runs locally in the Packet Tunnel extension and sends traffic only to the
servers the user configured. Browsing traffic and connection lists are never
recorded or transmitted by VoyaVPN. DNS queries are answered by the DNS servers
set in the app (Cloudflare DNS over HTTPS by default, reached through the
user's server) and are not logged by VoyaVPN.

The app keeps, on the device only: the user's server list and settings, per-
server byte counters, and a diagnostic log kept for 7 days with credentials
removed. The log leaves the device only when the user exports or shares it.

For what purposes are you collecting this information?
No information is collected. The only requests the app makes on its own are
functional:
- It downloads the subscription URLs the user enters, to get their servers.
- It downloads routing rule files from raw.githubusercontent.com.
- After connecting, it looks up the exit IP address and country through the
  user's own server (ipwho.is, icanhazip.com, ipify.org, ident.me), to show
  where traffic exits and whether the server supports IPv6. Nothing is sent
  besides the request itself.
- It measures latency with a request to www.google.com/generate_204 through
  the server being tested.
- On macOS only, when the user runs the self-hosted node's network check, it
  sends the list of chosen port numbers to probe.voyavpn.app, which tries to
  connect back to those ports and returns the result, and it asks
  www.cloudflare.com/cdn-cgi/trace for the Mac's public IP address. The probe
  service stores nothing and keeps no logs.

Will the data be shared with any third parties?
No. No user data is stored on any VoyaVPN server or shared with anyone. The
requests above go directly to the services named, carry no identifier or
personal data, and the traffic the user sends through the VPN goes only to the
proxy servers the user chose.
```

## Platform differences

The answers hold for the macOS and iOS builds. Two differences matter if a
reviewer asks:

- iOS has no self-hosted node, so it never listens for incoming connections
  and never calls `probe.voyavpn.app`. iOS has no App Sandbox entitlements, so
  the `network.server` question does not arise there.
- The Android build (not an App Store product) uses Google ML Kit for QR
  scanning, which sends usage metrics to Google; see
  [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Do not copy the answers
  above into a Google Play data-safety form without adding that.

## App Review Information → Notes

Paste the block for the platform into App Review Information → Notes on every
submission. Replace the two placeholders in REVIEW TEST CONFIGURATION with a
node the reviewer can reach from the United States for the whole review, and
never commit that credential. Each block is about 2,500 characters with a
typical share link; keep it under the field's 4,000.

### macOS

```text
APP PURPOSE
VoyaVPN is a VPN and proxy client for macOS by Beijing Wangcai Technology Co.,
Ltd. Users import server configurations or subscription URLs they own or are
authorized to use. The app includes no VPN service of its own and sells no
servers or plans. There is no account or sign-in. A "subscription" here is a
URL that lists servers, not an App Store subscription.

REVIEW TEST CONFIGURATION
Share link or subscription URL: <FILL IN BEFORE SUBMITTING>
Valid until: <DATE AFTER THE REVIEW ENDS>

TEST STEPS
1. Open Nodes > Add > Paste links or subscription URLs, paste the
   configuration above, and click Import.
2. Select the imported node and click Connect.
3. Allow macOS to add the VPN configuration. Home shows Connected.
4. Open any HTTPS website. Network activity lists the connections.
5. Disconnect from Home.

HOW THE APP USES THE SYSTEM
- The VPN is Apple's NetworkExtension Packet Tunnel provider, embedded as an
  app extension.
- The app never asks for administrator rights, installs no helper tool, and
  runs no sudo.
- Settings > Startup > Autostart registers a launchd agent bundled inside the
  app with SMAppService.
- Screen Recording is requested only by the optional "Scan screen" QR import.

WHY com.apple.security.network.server
Three features accept incoming connections:
1. Self-hosted node (optional): the user can turn this Mac into a proxy server
   for their own devices. The bundled sing-box, which inherits the app's
   sandbox, listens on the ports the user picks.
2. Latency test while disconnected: the app starts that sing-box with a proxy
   listener on 127.0.0.1 and measures servers through it. To see it, click
   Ping on any node while disconnected.
3. The Packet Tunnel's sing-box opens a proxy port and a control API on
   127.0.0.1, which the app reads to show connections.
Without the entitlement each fails with "bind: operation not permitted".

DATA
VoyaVPN collects no user data. It has no analytics, crash reporting,
advertising, telemetry or accounts. Traffic goes only to the servers the user
configured. The server list, settings and a 7-day diagnostic log with
credentials removed stay on the device. The full answers to the VPN questions
are in the privacy policy: https://voyavpn.wangc.ai/privacy

Store screenshots show demonstration data. Please use the configuration above
to test.
```

### iOS and iPadOS

```text
APP PURPOSE
VoyaVPN for iPhone and iPad is a VPN and proxy client by Beijing Wangcai
Technology Co., Ltd. Users import server configurations or subscription URLs
they own or are authorized to use. The app includes no VPN service of its own
and sells no servers or plans. There is no account or sign-in. A
"subscription" here is a URL that lists servers, not an App Store subscription.

TERRITORIES
VoyaVPN is not offered in China mainland, which is excluded from its
availability. It is offered only in territories that require no VPN licence.

REVIEW TEST CONFIGURATION
Share link or subscription URL: <FILL IN BEFORE SUBMITTING>
Valid until: <DATE AFTER THE REVIEW ENDS>

TEST STEPS
1. On first launch, read "Before you start" and tap Continue. The same
   information stays under Settings > About & help > Privacy information.
2. On Home, tap "Add nodes or subscription". Paste the configuration above,
   tap Preview, then Confirm import.
3. Open Nodes and select the imported node.
4. On Home, tap Connect and allow iOS to add the VPN configuration. Home
   shows Connected and the exit IP address.
5. Open any HTTPS website. Home > Network activity lists the connections.
6. Disconnect from Home.

HOW THE APP USES THE SYSTEM
- The VPN is Apple's NetworkExtension Packet Tunnel provider, embedded as an
  app extension. iOS asks once to add the VPN configuration.
- Camera is requested only by the optional "Scan QR code" import.
- Local Network is requested only when a latency test reaches a server the
  user added on their own network.
- The app asks for nothing else. It has no background modes, no push
  notifications and no location access, and it never accepts incoming
  connections.

DATA
VoyaVPN collects no user data. It has no analytics, crash reporting,
advertising, telemetry or accounts. Traffic goes only to the servers the user
configured. The server list, settings and a 7-day diagnostic log with
credentials removed stay on the device. The app states this on the "Before you
start" screen, and the full answers to the VPN questions are in the privacy
policy: https://voyavpn.wangc.ai/privacy

Store screenshots show demonstration data. Please use the configuration above
to test.
```

## Sales and territories

Guideline 5.4 requires a VPN app to provide its licence information in the
review notes for every territory that requires a VPN licence. VoyaVPN holds
none, so neither app is offered in China mainland.

Set this once per app record, for the macOS app and the iOS app: App Store
Connect > the app > **Pricing and Availability** > **App Availability** >
**Edit**, and clear **China mainland**. Check it again before each
submission, because a new app record starts with every territory selected. The
iOS notes block says so in its TERRITORIES paragraph. If a reviewer raises it
for the macOS app, reply with the same two sentences.

## Resolution Center reply for build 405

Post this in the App Store Connect thread for the rejected build before
submitting the new one. Replace `<BUILD>` with the new build number.

```text
Thank you for the review. Build <BUILD> replaces build 405 and addresses each
point:

1. Guideline 2.5.1, non-public API __kCFBundleNumericVersionKey: the symbol
   came from the bundled sing-box binary, whose upstream release links
   Chromium's Cronet. We now build sing-box from source without that
   component, and our packaging check rejects any binary that imports the
   symbol or links a private library.

2. com.apple.security.network.server: the entitlement is used. The optional
   self-hosted node listens for the user's own devices, and the latency test
   and the Packet Tunnel run local listeners on 127.0.0.1. The App Review
   notes explain each one and how to see it.

3. VPN information: VoyaVPN collects no user data. The answers are in the App
   Review notes and in our privacy policy, https://voyavpn.wangc.ai/privacy

We also removed anything that could look like privilege escalation. The app
never asks for administrator rights, installs no helper tool, and uses
SMAppService for launch at login. A working test configuration is in the App
Review notes.
```

## Resolution Center reply for build 407

Build 407 was rejected on 2026-10-05 under 2.1(a): "the button for showing the
main window in the menu bar extra app was unresponsive" (macOS 27.0). The plain
path, close the window and choose Show VoyaVPN, works on macOS 26.5. What does
reproduce on the sandboxed build is a second copy of the app: the
single-instance socket lives in `/tmp`, which the App Sandbox denies, so the
login agent, which starts the moment Launch at login is turned on, ran beside
the first copy with its own menu bar icon and a window stacked exactly on the
first. Closing or showing one window then changes nothing on screen. That this
is what the reviewer saw is an inference; ask for a screen recording if the next
build is stopped for the same reason. Replace `<BUILD>` with the new build
number.

```text
Thank you for the report. Build <BUILD> replaces build 407.

We reproduced a case in which the menu bar item appeared to do nothing: after
turning on "Launch at login", a second copy of the app started beside the
running one, each with its own menu bar icon and window. The windows sat
exactly on top of each other, so closing one or choosing "Show VoyaVPN"
changed nothing visible. Build <BUILD> allows only one running copy.

Build <BUILD> also fixes two related issues: the menu bar item now follows the
window when it is minimized or the app is hidden, so its first click always
acts, and closing the window while it is full screen no longer leaves an empty
full-screen Space behind.

If the item is still unresponsive on your device, a short screen recording of
the steps would help us, because we could not reproduce any other failure.
```

## Where the facts come from

| Claim | Source |
| --- | --- |
| Helper and node listeners | `crates/voya-core/src/singbox/inbounds.rs`, `crates/voya-core/src/singbox/selfhost.rs`, [ADR 0011](../adr/0011-self-hosted-node.md) |
| Provider listeners | `crates/voya-core/src/singbox/experimental.rs`, [macos-networkextension-troubleshooting.md](macos-networkextension-troubleshooting.md) |
| No telemetry SDK | `Cargo.lock`, `pnpm-lock.yaml`, `apps/mobile/ios/Podfile.lock` |
| Log retention and redaction | `crates/voya-app/src/logging.rs` |
| Endpoints | `crates/voya-core/src/singbox/mod.rs`, `crates/voya-net/src/probe/`, `crates/voya-app/src/ipv6_egress.rs`, `crates/voya-contracts/src/settings.rs` |
| Probe service stores nothing | [self-host-probe-worker.md](self-host-probe-worker.md), `apps/probe/wrangler.jsonc` |
| No privilege escalation on macOS | `crates/voya-platform/src/privilege/linux_installer.rs` (compiled for Linux only), [ADR 0004](../adr/0004-platform-boundaries.md), `scripts/native/macos/macho-imports.mjs` |
| Launch at login | `crates/voya-platform/native/macos_login_item.m`, `apps/desktop/src-tauri/native/macos/LaunchAgents/app.voyavpn.desktop.autostart.plist` |
| In-app data declaration (Guideline 5.4) | `apps/mobile/src/features/settings/privacy-notice.ts` and `privacy-notice-screen.tsx`, `packages/i18n/src/locales/*.json` (`mobile.privacyNotice*`) |
| iOS permissions | `apps/mobile/ios/VoyaVPN/Info.plist`, `apps/mobile/ios/VoyaVPN/VoyaVPN.entitlements`; checked by `vp run check mobile ios assets` |
| Privacy Policy page | `apps/web/src/content/*.ts`, checked against this page by `apps/web/test/privacy-sync.test.ts` |
| iOS privacy manifest | `apps/mobile/ios/VoyaVPN/PrivacyInfo.xcprivacy` (no collected data types, no tracking) |
