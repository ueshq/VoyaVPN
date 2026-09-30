# App Store Review Notes

This page holds the answers App Review asks for when VoyaVPN is submitted to
the Mac App Store, and later the iOS App Store. Each block below is written to
be pasted as-is into App Store Connect. Keep them true: update this page in the
same change that alters what the app listens on, connects to, or stores.

Where each block goes:

- **App Review Information → Notes** (App Store Connect, the version page):
  [the notes block](#app-review-information--notes), every submission. The
  field holds 4,000 characters, so that block summarizes the two long answers
  below instead of repeating them.
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
| Non-public API `__kCFBundleNumericVersionKey` (2.5.1) | Fixed in the binary: the bundled sing-box is built from source without the Cronet-based naive outbound. `native:macos:pkg` now refuses any Mach-O that imports it. |
| VPN information request | Reply with [the VPN answers](#vpn-questions). |
| Not raised yet, fixed before resubmitting (2.4.5) | macOS has no privilege-escalation path: the admin-prompt installer is compiled for Linux only, and `native:macos:pkg` refuses escalation text. Launch at login registers a bundled agent with `SMAppService` instead of writing `~/Library/LaunchAgents`. |

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

Paste this into App Review Information → Notes on every submission. Replace
the two placeholders in REVIEW TEST CONFIGURATION with a node the reviewer can
reach from the United States for the whole review, and never commit that
credential. The block is about 2,500 characters with a typical share link;
keep it under the field's 4,000.

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
| Privacy Policy page | `apps/web/src/content/*.ts`, checked against this page by `apps/web/test/privacy-sync.test.ts` |
| iOS privacy manifest | `apps/mobile/ios/VoyaVPN/PrivacyInfo.xcprivacy` (no collected data types, no tracking) |
