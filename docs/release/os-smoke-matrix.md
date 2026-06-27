# Release OS Smoke Matrix

Batch: `05-03-stable-runbooks-and-smoke`

These checks must run on real operating systems before production stable publication. The generated runner does not execute external publication, CDN pointer promotion, signing, notarization, diagnostics approval, or real OS smoke.

## Evidence Links

- Top-level release path: [runbook.md](runbook.md)
- Signing and updater prerequisites: [signing-notarization.md](signing-notarization.md)
- Rollback procedures: [rollback.md](rollback.md)
- Stable external evidence checklist: [external-evidence-checklist.md](external-evidence-checklist.md)
- Stable gate: [../verification/stable-release-gate.md](../verification/stable-release-gate.md)
- Diagnostics privacy contract: [diagnostics-privacy.md](diagnostics-privacy.md)

## Evidence Required For Every OS Run

Record:

- Operator and owner role.
- Commit SHA, version, channel, artifact name, and SHA-256.
- OS name, version, architecture, desktop environment when relevant, and clean-user status.
- Install mode: unsigned debug package, signed package, or release build.
- CDN release index entry, updater metadata entry, core manifest entry, and artifact URL host for stable runs.
- Core binaries used, versions, paths, and whether they were downloaded on first run or preinstalled.
- Redacted test server or subscription source.
- Diagnostics setting state, redacted release-health event result, and opt-out result when diagnostics smoke runs.
- Before and after OS proxy, routes, TUN devices, autostart entries, hotkeys, and running process state.
- Logs, screenshots, terminal output, and exact commands proving pass or fail.
- Skipped checks with concrete blocker, owner, and follow-up.

## Stable Target Coverage

The first production stable matrix covers x64 and arm64 for Windows, macOS, and Linux. Each target must have manual download smoke, updater smoke, core smoke, diagnostics smoke, and rollback readiness evidence before pointer promotion. Release owners record the target artifact names, SHA-256 values, signature/notarization evidence, smoke logs, screenshots, and stop or rollback decision in [external-evidence-checklist.md](external-evidence-checklist.md).

| Stable target | Owner | System | Required verification | Rollback or stop condition |
| --- | --- | --- | --- | --- |
| `windows-x86_64` | Windows platform owner | Clean Windows x64 smoke machine, signed NSIS/MSI, stable CDN package and updater entries | Install, launch, manual download checksum/signature validation, updater smoke from older signed build, Xray/mihomo/sing-box core smoke, diagnostics smoke, proxy/TUN cleanup, uninstall. | Hold or roll back Windows x64 release-index and updater entries; restore OS proxy/routes and quarantine bad artifacts. |
| `windows-aarch64` | Windows platform owner | Clean Windows arm64 smoke machine, signed arm64 NSIS/MSI, stable CDN package and updater entries | Native arm64 install and launch, manual download checksum/signature validation, updater smoke, arm64 core smoke, diagnostics smoke, proxy/TUN cleanup, uninstall. | Hold or roll back `windows-aarch64` release-index and updater entries; restore OS state and quarantine bad artifacts. |
| `darwin-x86_64` | macOS platform owner | Clean Intel macOS smoke machine, signed/notarized/stapled DMG, stable CDN package and updater entries | Gatekeeper launch, manual download checksum/notarization validation, updater smoke, x64 core smoke, diagnostics smoke, proxy/TUN cleanup, uninstall. | Hold or roll back `darwin-x86_64` release-index and updater entries; remove app bundle and restore OS state. |
| `darwin-aarch64` | macOS platform owner | Clean Apple Silicon macOS smoke machine, signed/notarized/stapled arm64 DMG, stable CDN package and updater entries | Gatekeeper launch, manual download checksum/notarization validation, updater smoke, arm64 core smoke, diagnostics smoke, proxy/TUN cleanup, uninstall. | Hold or roll back `darwin-aarch64` release-index and updater entries; remove app bundle and restore OS state. |
| `linux-x86_64` | Linux platform owner | Clean Linux x64 smoke machines for `.deb`, `.rpm`, and `.AppImage`, stable CDN package and updater entries | Package install or AppImage launch, manual download checksum validation, updater smoke where supported, x64 core smoke, diagnostics smoke, proxy/TUN cleanup, uninstall/removal. | Hold or roll back `linux-x86_64` release-index and updater entries; revert package repository metadata if used; restore OS state. |
| `linux-aarch64` | Linux platform owner | Clean Linux arm64 smoke machines for `.deb`, `.rpm`, and `.AppImage`, stable CDN package and updater entries | Package install or AppImage launch, manual download checksum validation, updater smoke where supported, arm64 core smoke, diagnostics smoke, proxy/TUN cleanup, uninstall/removal. | Hold or roll back `linux-aarch64` release-index and updater entries; revert package repository metadata if used; restore OS state. |

## Platform Matrix

| Platform | Owner | System | Required verification | Rollback or stop condition |
| --- | --- | --- | --- | --- |
| Windows 11 x64 | Windows platform owner | Signed NSIS and MSI packages on clean Windows 11 | Install current-user package, launch, first-run core acquisition, real connect, logs and stats, forced system proxy, PAC mode, TUN/UAC cleanup, autostart, hotkeys, updater detection, uninstall, no orphaned processes. | Restore proxy and routes, uninstall package, remove autostart and hotkeys, stop updater publication for Windows if failure is release-blocking. |
| Windows 11 arm64 | Windows platform owner | Signed arm64 NSIS and MSI packages on clean Windows 11 arm64 | Install current-user package, launch native arm64 app, first-run arm64 core acquisition, real connect, proxy and TUN/UAC cleanup, updater detection, uninstall, no orphaned processes. | Restore proxy and routes, uninstall package, hold `windows-aarch64` artifacts and updater payloads. |
| Windows 10 x64 | Windows platform owner | Signed package on clean Windows 10 | Launch, real connect, proxy restore, WebView2 bootstrap behavior, updater detection if supported, uninstall cleanup. | Restore OS state, uninstall package, hold Windows 10 support claim or pull Windows assets. |
| macOS Apple Silicon arm64 | macOS platform owner | Signed, notarized, and stapled arm64 DMG on clean Apple Silicon macOS | Gatekeeper launch, first-run arm64 core acquisition, real connect, proxy restore, sudo TUN cleanup, LaunchAgent autostart, hotkeys, updater detection, uninstall cleanup. | Remove app bundle, restore proxy/routes/TUN state, hold `darwin-aarch64` artifacts and updater payloads. |
| macOS Intel x64 | macOS platform owner | Signed, notarized, and stapled x64 DMG on Intel macOS | Gatekeeper launch, real connect, proxy restore, first-run x64 core acquisition, updater detection, uninstall cleanup. | Hold `darwin-x86_64` support claim or pull Intel macOS assets. |
| Linux Debian-like x64 | Linux platform owner | `.deb` package on clean supported distribution | Install, desktop entry, launch, first-run core acquisition, real connect, proxy shell restore, sudo TUN cleanup, autostart, hotkeys, uninstall cleanup. | Remove package, restore proxy/routes/TUN state, republish previous package index if already staged. |
| Linux RPM-like x64 | Linux platform owner | `.rpm` package on clean supported distribution | Install, desktop entry, launch, real connect, proxy restore, sudo TUN cleanup, uninstall cleanup. | Remove package, restore OS state, hold RPM publication. |
| Linux AppImage x64 | Linux platform owner | `.AppImage` on clean supported distribution | Execute bit, launch, config directory creation, first-run core acquisition, real connect, proxy restore, sudo TUN cleanup. | Delete AppImage, restore OS state, hold AppImage publication. |
| Linux Debian-like arm64 | Linux platform owner | arm64 `.deb` package on clean supported distribution | Install, desktop entry, launch, first-run arm64 core acquisition, real connect, proxy shell restore, sudo TUN cleanup, uninstall cleanup. | Remove package, restore OS state, hold `linux-aarch64` package publication. |
| Linux RPM-like arm64 | Linux platform owner | arm64 `.rpm` package on clean supported distribution | Install, desktop entry, launch, real connect, proxy restore, sudo TUN cleanup, uninstall cleanup. | Remove package, restore OS state, hold arm64 RPM publication. |
| Linux AppImage arm64 | Linux platform owner | arm64 `.AppImage` on clean supported distribution | Execute bit, launch, config directory creation, first-run arm64 core acquisition, real connect, proxy restore, sudo TUN cleanup. | Delete AppImage, restore OS state, hold arm64 AppImage publication. |

## Smoke Checkpoints

| Checkpoint | Owner | System | Verification | Rollback or stop condition |
| --- | --- | --- | --- | --- |
| Artifact integrity | Platform owner | Downloaded package and `SHA256SUMS` | Local SHA-256 matches the release artifact manifest. | Delete the local artifact and re-download. If hosted checksum differs, stop publication and republish correct assets. |
| Manual download smoke | Platform owner | Stable CDN release index, platform package URL, checksum and signature evidence | The release index advertises the correct OS/arch entry, the package downloads from the approved CDN, SHA-256 matches, signature/notarization/package trust validates, and install or launch begins from that artifact. | Stop pointer promotion or restore the previous release-index pointer for the affected target; quarantine the bad artifact with its hash. |
| Install and launch | Platform owner | OS installer, app bundle, or AppImage | Package installs or opens cleanly, main window and tray appear, config directories are created, and no startup errors appear. | Uninstall or delete app bundle/AppImage, remove config artifacts only if this was a clean test account, and hold the platform package. |
| First-run core acquisition | Platform owner | In-app update manager and app data `bin/` tree | Selected cores and geo/ruleset files download or the missing-core error is typed and actionable. Included seed assets are limited to approved Xray, mihomo, and sing-box entries and are copied into app data before execution. | Delete staged core downloads, restore previous app data snapshot, and stop publication if installers include unapproved core assets. |
| Core smoke | Platform owner and release engineer | Core manifest, staged downloads, app data `bin/`, runtime supervisor | Xray, mihomo, and sing-box check/download/apply paths verify checksum, staged extraction, Unix chmod, safe swap, rollback on failed apply, restart behavior, and no execution from the read-only app bundle. | Restore the previous core manifest pointer, keep app-data backup directories, quarantine bad core archives, and block the affected OS/arch. |
| Real connection | Platform owner | Real redacted server, Xray and sing-box where supported | Add/import a profile, connect, traffic exits through local inbound, logs stream, status changes, stats update, and disconnect stops the core. | Disconnect, kill orphaned core processes, restore proxy/routes, and attach logs to the release issue. |
| System proxy restore | Platform owner | OS proxy settings and VoyaVPN proxy modes | Record before state, enable forced change, confirm browser/curl traffic, switch forced clear or unchanged, quit while enabled, and verify restoration. Windows PAC must stop when switching away. | Restore proxy manually from recorded before state and block publication if automatic restore fails. |
| TUN and elevation cleanup | Platform owner | OS routes, DNS, TUN devices, sudo or UAC | Record before state, enable TUN, confirm traffic and DNS, disable, quit, and verify routes, DNS, devices, and elevated processes are cleaned up. | Disable TUN, remove stale routes/devices, kill elevated helpers, and block publication for the affected platform. |
| Runtime supervisor cleanup | Platform owner | Core process tree and logs | Main and pre processes start and stop in expected order; crash or forced stop does not leave orphaned child or elevated processes. | Kill leftover processes, collect logs, and block publication if cleanup is not deterministic. |
| Autostart and hotkeys | Platform owner | Registry, LaunchAgent, desktop autostart file, global hotkey registration | Enable, inspect OS artifact, trigger each hotkey outside the app window, disable, and verify cleanup. | Remove OS autostart artifacts and hotkey registrations manually. Hold publication if cleanup fails. |
| Backup and WebDAV, when credentials exist | Platform owner | Local backup and WebDAV endpoint | Local backup round-trips; WebDAV push and pull restore into a clean profile state with credentials redacted. | Delete test remote artifacts, restore local backup, and record an owner-approved stable skip if credentials were unavailable. |
| Updater smoke | Release engineer and platform owner | Older signed build and stable updater endpoint | Older build detects the new stable version for its exact target, signature validates, update applies, app launches, and version changes. | Re-publish previous `latest.json` pointer or remove stable metadata. Keep direct downloads only if approved. |
| Diagnostics smoke | Privacy/security owner and platform owner | Stable diagnostics setting, event envelope, endpoint or approved disablement control | Default-on state is visible, opt-out persists and clears pending events, redacted release-health events deliver or are disabled by approved control, and forbidden fields are absent. | Disable diagnostics delivery through the approved control path and block publication if node URLs, credentials, IP addresses, full logs, generated configs, or traffic destinations can be emitted. |
| Uninstall cleanup | Platform owner | OS package manager or app removal flow | App removes cleanly, no orphaned process remains, and OS proxy/TUN/autostart/hotkey state is restored. | Remove leftovers manually and block publication if uninstall damages OS state. |

## v2rayN Parity Smoke Addendum

These checks cover parity features that are user-visible in v2rayN but implemented in VoyaVPN's Tauri/Rust architecture. Record pass, fail, or a platform-specific unavailable reason for each supported OS.

| Area | Verification |
| --- | --- |
| Full config templates | Open Settings, then Full Config Template. Edit Xray and sing-box JSON object templates, toggle Enabled/Add proxy only, set Proxy detour, save, reopen, and verify persisted values. Connect one Xray and one sing-box profile that should consume the enabled template. |
| Certificate fetch | Open a TLS profile, fetch leaf cert, fetch chain, calculate SHA from pasted PEM, and save. Verify self-signed or invalid chains fail by default and succeed only when Allow insecure fetch is explicitly enabled for the fetch action. |
| QR import | Import from image file, clipboard text, clipboard image, and screen scan. On macOS screen-recording restrictions or Linux Wayland limitations, record the exact unavailable message and confirm image/clipboard paths still work. |
| Share/export | Select one and multiple profiles. Export share links, base64 share links, inner links, and client config from toolbar and row context menu. Confirm clipboard contents and QR dialog display match the selected profile order. |
| Settings coverage | In Settings, change Core basic, Mux, TUN, System proxy, Speed test, Hysteria, Fragment, Update source, and CoreType mapping values. Save, reopen, and verify persisted `AppConfig` values without schema migration. |
| Core acquisition boundary | In Updates, verify Xray, mihomo, and sing-box show automatic update/download behavior. Other listed cores must show manual or unsupported acquisition state and require user-supplied binaries or a separately approved package path. |
| End-to-end runtime | Connect and disconnect a redacted profile after settings/template edits, verify system proxy restore, TUN cleanup, logs, runtime state, speed test, backup create/restore, and no orphaned core process. |

## Pass Criteria

A platform passes release smoke when:

- Signed or approved package installs or launches on a clean account.
- A real server can connect and pass traffic.
- Logs and runtime state are visible.
- System proxy and TUN restore OS state after disconnect, quit, and practical failure paths.
- First-run core acquisition and core smoke pass with only approved Xray, mihomo, and sing-box seed or CDN core assets.
- Updater metadata is real, signed, and hosted on the approved CDN for release builds.
- Manual download smoke verifies CDN release-index entries for the target artifact.
- Diagnostics smoke verifies default-on behavior, opt-out, and redaction.
- Uninstall or app removal leaves no orphaned process, proxy setting, route, TUN device, autostart entry, or hotkey.

Any release-blocking failure must be linked from the stable release evidence or issue tracker before publication continues.
