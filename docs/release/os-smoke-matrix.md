# Release OS Smoke Matrix

Batch: `05-03-stable-runbooks-and-smoke`

These checks must run on real operating systems before production stable publication. The generated runner does not execute external publication, CDN pointer promotion, signing, notarization, or real OS smoke.

## Evidence Links

- Top-level release path: [runbook.md](runbook.md)
- Signing and updater prerequisites: [signing-notarization.md](signing-notarization.md)
- Rollback procedures: [rollback.md](rollback.md)
- Stable external evidence checklist: [external-evidence-checklist.md](external-evidence-checklist.md)
- Stable gate: [../verification/stable-release-gate.md](../verification/stable-release-gate.md)

## Evidence Required For Every OS Run

Record:

- Operator and owner role.
- Commit SHA, version, channel, artifact name, and SHA-256.
- OS name, version, architecture, desktop environment when relevant, and clean-user status.
- Install mode: unsigned debug package, signed package, or release build.
- CDN release index entry, updater metadata entry, core manifest entry, and artifact URL host for stable runs.
- Core binaries used, versions, and paths, plus the packaged seed SHA-256 they were copied from. Every package bundles the sing-box seed (on macOS it only backs disconnected speedtests; the PacketTunnel's Libbox runs the connection); the app has no first-run download path, so a missing core means a package built without a staged seed, not a failed download.
- Redacted test server or subscription source.
- Before and after OS proxy, routes, TUN devices, autostart entries, and running process state.
- Logs, screenshots, terminal output, and exact commands proving pass or fail.
- Skipped checks with concrete blocker, owner, and follow-up.

## Stable Target Coverage

The first production stable matrix covers x64 and arm64 for Windows, macOS, and Linux. Each target must have manual download smoke, updater smoke, core smoke, and rollback readiness evidence before pointer promotion. Release owners record the target artifact names, SHA-256 values, signature/notarization evidence, smoke logs, screenshots, and stop or rollback decision in [external-evidence-checklist.md](external-evidence-checklist.md).

| Stable target | Owner | System | Required verification | Rollback or stop condition |
| --- | --- | --- | --- | --- |
| `windows-x86_64` | Windows platform owner | Clean Windows x64 smoke machine, signed NSIS/MSI, stable CDN package and updater entries | Install, launch, manual download checksum/signature validation, updater smoke from older signed build, sing-box core smoke, proxy/TUN cleanup, uninstall. | Hold or roll back Windows x64 release-index and updater entries; restore OS proxy/routes and quarantine bad artifacts. |
| `windows-aarch64` | Windows platform owner | Clean Windows arm64 smoke machine, signed arm64 NSIS/MSI, stable CDN package and updater entries | Native arm64 install and launch, manual download checksum/signature validation, updater smoke, arm64 core smoke, proxy/TUN cleanup, uninstall. | Hold or roll back `windows-aarch64` release-index and updater entries; restore OS state and quarantine bad artifacts. |
| `darwin-x86_64` | macOS platform owner | Clean Intel macOS smoke machine, signed/notarized/stapled DMG, stable CDN package and updater entries | Gatekeeper launch, `pnpm native:macos:tunnel:verify` evidence for static Libbox symbols or embedded Libbox plus provisioning profiles and entitlements, manual download checksum/notarization validation, updater smoke, x64 core smoke, proxy/TUN cleanup, uninstall. | Hold or roll back `darwin-x86_64` release-index and updater entries; remove app bundle and restore OS state. |
| `darwin-aarch64` | macOS platform owner | Clean Apple Silicon macOS smoke machine, signed/notarized/stapled arm64 DMG, stable CDN package and updater entries | Gatekeeper launch, `pnpm native:macos:tunnel:verify` evidence for static Libbox symbols or embedded Libbox plus provisioning profiles and entitlements, manual download checksum/notarization validation, updater smoke, arm64 core smoke, proxy/TUN cleanup, uninstall. | Hold or roll back `darwin-aarch64` release-index and updater entries; remove app bundle and restore OS state. |
| `linux-x86_64` | Linux platform owner | Clean Linux x64 smoke machines for `.deb`, `.rpm`, and `.AppImage`, stable CDN package and updater entries | Package install or AppImage launch, manual download checksum validation, updater smoke where supported, x64 core smoke, proxy/TUN cleanup, uninstall/removal. | Hold or roll back `linux-x86_64` release-index and updater entries; revert package repository metadata if used; restore OS state. |
| `linux-aarch64` | Linux platform owner | Clean Linux arm64 smoke machines for `.deb`, `.rpm`, and `.AppImage`, stable CDN package and updater entries | Package install or AppImage launch, manual download checksum validation, updater smoke where supported, arm64 core smoke, proxy/TUN cleanup, uninstall/removal. | Hold or roll back `linux-aarch64` release-index and updater entries; revert package repository metadata if used; restore OS state. |

## Platform Matrix

| Platform | Owner | System | Required verification | Rollback or stop condition |
| --- | --- | --- | --- | --- |
| Windows 11 x64 | Windows platform owner | Signed NSIS and MSI packages on clean Windows 11 | Install current-user package, launch, first-run core seed staging, real connect, logs and stats, forced system proxy, TUN/UAC cleanup, autostart, updater detection, uninstall, no orphaned processes. | Restore proxy and routes, uninstall package, remove autostart, stop updater publication for Windows if failure is release-blocking. |
| Windows 11 arm64 | Windows platform owner | Signed arm64 NSIS and MSI packages on clean Windows 11 arm64 | Install current-user package, launch native arm64 app, first-run arm64 core seed staging, real connect, proxy and TUN/UAC cleanup, updater detection, uninstall, no orphaned processes. | Restore proxy and routes, uninstall package, hold `windows-aarch64` artifacts and updater payloads. |
| Windows 10 x64 | Windows platform owner | Signed package on clean Windows 10 | Launch, real connect, proxy restore, WebView2 bootstrap behavior, updater detection if supported, uninstall cleanup. | Restore OS state, uninstall package, hold Windows 10 support claim or pull Windows assets. |
| macOS Apple Silicon arm64 | macOS platform owner | Signed, notarized, and stapled arm64 DMG on clean Apple Silicon macOS | Gatekeeper launch, Libbox runtime, provisioning profile, and PacketTunnel entitlement verification, signed `Contents/Resources/core-seeds/sing_box/sing-box` in the bundle, latency test of several nodes while disconnected, real connect, latency test of several nodes while connected, terminal traffic through `claude`/`codex` without proxy env vars, proxy restore, PacketTunnel VPN authorization and cleanup, LaunchAgent autostart, updater detection, uninstall cleanup. | Remove app bundle, restore proxy/routes/TUN state, hold `darwin-aarch64` artifacts and updater payloads. |
| macOS Intel x64 | macOS platform owner | Signed, notarized, and stapled x64 DMG on Intel macOS | Gatekeeper launch, Libbox runtime, provisioning profile, and PacketTunnel entitlement verification, real connect, terminal traffic through `claude`/`codex` without proxy env vars, proxy restore, signed `Contents/Resources/core-seeds/sing_box/sing-box` in the bundle, latency test while disconnected and while connected, updater detection, uninstall cleanup. | Hold `darwin-x86_64` support claim or pull Intel macOS assets. |
| Linux Debian-like x64 | Linux platform owner | `.deb` package on clean supported distribution | Install, desktop entry, launch, first-run core seed staging, real connect, proxy shell restore, sudo TUN cleanup, autostart, uninstall cleanup. | Remove package, restore proxy/routes/TUN state, republish previous package index if already staged. |
| Linux RPM-like x64 | Linux platform owner | `.rpm` package on clean supported distribution | Install, desktop entry, launch, real connect, proxy restore, sudo TUN cleanup, uninstall cleanup. | Remove package, restore OS state, hold RPM publication. |
| Linux AppImage x64 | Linux platform owner | `.AppImage` on clean supported distribution | Execute bit, launch, config directory creation, first-run core seed staging, real connect, proxy restore, sudo TUN cleanup. | Delete AppImage, restore OS state, hold AppImage publication. |
| Linux Debian-like arm64 | Linux platform owner | arm64 `.deb` package on clean supported distribution | Install, desktop entry, launch, first-run arm64 core seed staging, real connect, proxy shell restore, sudo TUN cleanup, uninstall cleanup. | Remove package, restore OS state, hold `linux-aarch64` package publication. |
| Linux RPM-like arm64 | Linux platform owner | arm64 `.rpm` package on clean supported distribution | Install, desktop entry, launch, real connect, proxy restore, sudo TUN cleanup, uninstall cleanup. | Remove package, restore OS state, hold arm64 RPM publication. |
| Linux AppImage arm64 | Linux platform owner | arm64 `.AppImage` on clean supported distribution | Execute bit, launch, config directory creation, first-run arm64 core seed staging, real connect, proxy restore, sudo TUN cleanup. | Delete AppImage, restore OS state, hold arm64 AppImage publication. |

## Smoke Checkpoints

| Checkpoint | Owner | System | Verification | Rollback or stop condition |
| --- | --- | --- | --- | --- |
| Artifact integrity | Platform owner | Downloaded package and `SHA256SUMS` | Local SHA-256 matches the release artifact manifest. | Delete the local artifact and re-download. If hosted checksum differs, stop publication and republish correct assets. |
| Manual download smoke | Platform owner | Stable CDN release index, platform package URL, checksum and signature evidence | The release index advertises the correct OS/arch entry, the package downloads from the approved CDN, SHA-256 matches, signature/notarization/package trust validates, and install or launch begins from that artifact. | Stop pointer promotion or restore the previous release-index pointer for the affected target; quarantine the bad artifact with its hash. |
| Install and launch | Platform owner | OS installer, app bundle, or AppImage | Package installs or opens cleanly, main window and tray appear, config directories are created, and no startup errors appear. | Uninstall or delete app bundle/AppImage, remove config artifacts only if this was a clean test account, and hold the platform package. |
| First-run core acquisition | Platform owner | Bundled resources and app data `bin/` tree | The sing-box seed is copied into app data before execution; geo and ruleset files download separately; missing-core errors are typed and actionable. Installers include only approved sing-box seed assets. | Restore previous app data snapshot and stop publication if installers include unapproved core assets. |
| Core smoke | Platform owner and release engineer | sing-box seed resource, empty core manifest, app data `bin/`, runtime supervisor | sing-box seed copy, executable permissions, restart behavior, no core download or update targets, and no execution from the read-only app bundle. | Restore the previous app package or manifest pointer, keep app-data backup directories, quarantine bad package resources, and block the affected OS/arch. |
| Real connection | Platform owner | Real redacted server and sing-box | Add/import a profile, connect, traffic exits through local inbound, logs stream, status changes, stats update, and disconnect stops the core. | Disconnect, kill orphaned core processes, restore proxy/routes, and attach logs to the release issue. |
| System proxy restore | Platform owner | OS proxy settings and VoyaVPN proxy modes | Record before state, enable forced change, confirm browser/curl traffic, switch forced clear or unchanged, quit while enabled, and verify restoration. | Restore proxy manually from recorded before state and block publication if automatic restore fails. |
| TUN and elevation cleanup | Platform owner | OS routes, DNS, TUN devices, PacketTunnel, Windows service, sudo, or UAC | Record before state, enable TUN, confirm browser and terminal traffic, confirm DNS, disable, quit, and verify routes, DNS, devices, VPN/service state, and elevated processes are cleaned up. macOS evidence must include Libbox runtime verification, embedded provisioning profiles, signed PacketTunnel entitlement, VPN authorization, and `claude`/`codex` traffic without proxy environment variables. | Disable TUN, remove stale routes/devices, stop PacketTunnel or Windows service, kill elevated helpers, and block publication for the affected platform. |
| Runtime supervisor cleanup | Platform owner | Core process tree and logs | Main and pre processes start and stop in expected order; crash or forced stop does not leave orphaned child or elevated processes. | Kill leftover processes, collect logs, and block publication if cleanup is not deterministic. |
| Autostart | Platform owner | Registry, LaunchAgent, desktop autostart file | Enable, inspect the OS artifact, verify launch at login, disable, and verify cleanup. | Remove OS autostart artifacts manually. Hold publication if cleanup fails. |
| Updater smoke | Release engineer and platform owner | Older signed build and stable updater endpoint | Older build detects the new stable version for its exact target, signature validates, update applies, app launches, and version changes. | Re-publish previous `latest.json` pointer or remove stable metadata. Keep direct downloads only if approved. |
| Uninstall cleanup | Platform owner | OS package manager or app removal flow | App removes cleanly, no orphaned process remains, and OS proxy/TUN/autostart state is restored. | Remove leftovers manually and block publication if uninstall damages OS state. |
| Desktop residency | Platform owner | Every OS package with a tray | With close action "Keep running in the tray", closing the window leaves the app and connection running and the tray (Windows: left click; macOS/Linux: menu) restores it; "Ask" shows the prompt and "Remember my choice" persists; macOS dock click reopens a hidden window. | Hold the release if a closed window cannot be restored or quitting leaves the core, proxy or TUN behind. |
| Tray controls | Platform owner | Every OS package with a tray | Tray Connect/Disconnect, Traffic Mode (Rule/Global), Nodes and (once a group exists) Policy Groups submenus change the running app and stay in sync with in-app changes; the active node is checked; a failed action raises a notice. | Hold the release if the tray acts on stale state or a failed action is silent. |
| Single instance and login launch | Platform owner | Every OS package | Launching a second copy focuses the running window instead of starting another; with autostart and "Start hidden in the tray" enabled, a login launch starts hidden (entry carries `--autostart`) while a manual launch shows the window. | Hold the release if two instances can run or a login launch shows no tray. |

## v2rayN Parity Smoke Addendum

These checks cover parity features that are user-visible in v2rayN but implemented in VoyaVPN's Tauri/Rust architecture. Record pass, fail, or a platform-specific unavailable reason for each supported OS.

| Area | Verification |
| --- | --- |
| Policy groups | Create selector, lowest-latency and failover groups on the Nodes page. Use connects through the group; the Policy groups tab in Network activity shows the current member and delays; a selector switch applies without reconnecting; Home and the tray show the active group. A subscription's first import adds one Auto group that is never activated and does not come back after deletion. |
| QR import | Open Profiles > Import and import from an image QR code, clipboard text, and screen scan. Clipboard text and screen scan must import immediately without an app dialog; image imports retain their editable preview. Screen scan hides VoyaVPN only while capturing, restores its original window state before decoding, scans every display, and imports unique nodes from all QR codes. Check multiple codes, duplicate nodes, different display scales, denied permissions, blank screens, partial capture failures and the 15-second timeout. Every error must restore the window and appear inline. A timed-out native worker must prevent a second capture until it exits. On macOS screen-recording restrictions or Linux Wayland limitations, record the exact localized error; there must be no browser screen-picker fallback. System authorization UI is allowed. See [screen-qr-import.md](screen-qr-import.md). |
| Share/export | In Profiles, export share links from the row context menu and the group header. Confirm the Nodes toolbar has no Export menu and no Base64, node-bundle or file-save export is offered. Confirm collapsed groups export all members and no client-config export is offered. Confirm clipboard contents and the read-only Show QR dialog match the selected profile order. |
| Settings coverage | In Settings, change Core basic, sing-box Mux, TUN, System proxy, Speed test, Hysteria, fragment master switch, and update sources. Save, reopen, and verify persisted `AppConfig` values without schema migration. Confirm Xray-only Mux/fragment details and client CDN inputs are absent. |
| Update boundary | In Updates, verify the signed app updater, Geo update, and SRS update are three independent actions. Confirm there is no pre-release preference, batch selection, manual client CDN fallback, or core download/update target. |
| End-to-end runtime | Connect and disconnect a redacted profile after settings/routing edits, verify system proxy restore, TUN cleanup, logs, runtime state, speed test, and no orphaned core process. |

## Pass Criteria

A platform passes release smoke when:

- Signed or approved package installs or launches on a clean account.
- A real server can connect and pass traffic.
- Logs and runtime state are visible.
- System proxy and TUN restore OS state after disconnect, quit, and practical failure paths.
- First-run core acquisition and core smoke pass with only the approved bundled sing-box seed.
- Updater metadata is real, signed, and hosted on the approved CDN for release builds.
- Manual download smoke verifies CDN release-index entries for the target artifact.
- Uninstall or app removal leaves no orphaned process, proxy setting, route, TUN device, or autostart entry.

Any release-blocking failure must be linked from the stable release evidence or issue tracker before publication continues.
