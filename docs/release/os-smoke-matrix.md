# Release OS Smoke Matrix

Batch: `08-03-release-runbooks`

These checks must run on real operating systems before public beta publication. Use [../verification/manual-os-smoke.md](../verification/manual-os-smoke.md) as the evidence template and [../verification/cross-platform-smoke.md](../verification/cross-platform-smoke.md) for the automated smoke boundary.

## Evidence Links

- Top-level release path: [runbook.md](runbook.md)
- Signing and updater prerequisites: [signing-notarization.md](signing-notarization.md)
- Rollback procedures: [rollback.md](rollback.md)
- Update subsystem evidence: [../verification/updates.md](../verification/updates.md)

## Evidence Required For Every OS Run

Record:

- Operator and owner role.
- Commit SHA, version, channel, artifact name, and SHA-256.
- OS name, version, architecture, desktop environment when relevant, and clean-user status.
- Install mode: unsigned debug package, signed package, or release build.
- Core binaries used, versions, paths, and whether they were downloaded on first run or preinstalled.
- Redacted test server or subscription source.
- Before and after OS proxy, routes, TUN devices, autostart entries, hotkeys, and running process state.
- Logs, screenshots, terminal output, and exact commands proving pass or fail.
- Skipped checks with concrete blocker, owner, and follow-up.

## Platform Matrix

| Platform | Owner | System | Required verification | Rollback notes |
| --- | --- | --- | --- | --- |
| Windows 11 x64 | Windows platform owner | Signed NSIS and MSI packages on clean Windows 11 | Install current-user package, launch, first-run core acquisition, real connect, logs and stats, forced system proxy, PAC mode, TUN/UAC cleanup, autostart, hotkeys, updater detection, uninstall, no orphaned processes. | Restore proxy and routes, uninstall package, remove autostart and hotkeys, stop updater publication for Windows if failure is release-blocking. |
| Windows 10 x64 | Windows platform owner | Signed package on clean Windows 10 | Launch, real connect, proxy restore, WebView2 bootstrap behavior, updater detection if supported, uninstall cleanup. | Restore OS state, uninstall package, hold Windows 10 support claim or pull Windows assets. |
| macOS Apple Silicon | macOS platform owner | Signed, notarized, and stapled DMG on clean Apple Silicon macOS | Gatekeeper launch, first-run core acquisition, real connect, proxy restore, sudo TUN cleanup, LaunchAgent autostart, hotkeys, updater detection, uninstall cleanup. | Remove app bundle, restore proxy/routes/TUN state, pull macOS assets or keep DMG staged. |
| macOS Intel, if supported | macOS platform owner | Signed, notarized, and stapled DMG on Intel macOS | Gatekeeper launch, real connect, proxy restore, first-run core acquisition, updater detection, uninstall cleanup. | Hold Intel support claim or pull Intel/universal macOS assets. |
| Linux Debian-like x64 | Linux platform owner | `.deb` package on clean supported distribution | Install, desktop entry, launch, first-run core acquisition, real connect, proxy shell restore, sudo TUN cleanup, autostart, hotkeys, uninstall cleanup. | Remove package, restore proxy/routes/TUN state, republish previous package index if already staged. |
| Linux RPM-like x64 | Linux platform owner | `.rpm` package on clean supported distribution | Install, desktop entry, launch, real connect, proxy restore, sudo TUN cleanup, uninstall cleanup. | Remove package, restore OS state, hold RPM publication. |
| Linux AppImage x64 | Linux platform owner | `.AppImage` on clean supported distribution | Execute bit, launch, config directory creation, first-run core acquisition, real connect, proxy restore, sudo TUN cleanup. | Delete AppImage, restore OS state, hold AppImage publication. |

## Smoke Checkpoints

| Checkpoint | Owner | System | Verification | Rollback notes |
| --- | --- | --- | --- | --- |
| Artifact integrity | Platform owner | Downloaded package and `SHA256SUMS` | Local SHA-256 matches the release artifact manifest. | Delete the local artifact and re-download. If hosted checksum differs, stop publication and republish correct assets. |
| Install and launch | Platform owner | OS installer, app bundle, or AppImage | Package installs or opens cleanly, main window and tray appear, config directories are created, and no startup errors appear. | Uninstall or delete app bundle/AppImage, remove config artifacts only if this was a clean test account, and hold the platform package. |
| First-run core acquisition | Platform owner | In-app update manager and app data `bin/` tree | Selected cores and geo/ruleset files download or the missing-core error is typed and actionable. Default installers do not contain GPL or AGPL cores. | Delete staged core downloads, restore previous app data snapshot, and stop publication if default installers bundled restricted cores. |
| Real connection | Platform owner | Real redacted server, Xray and sing-box where supported | Add/import a profile, connect, traffic exits through local inbound, logs stream, status changes, stats update, and disconnect stops the core. | Disconnect, kill orphaned core processes, restore proxy/routes, and attach logs to the release issue. |
| System proxy restore | Platform owner | OS proxy settings and VoyaVPN proxy modes | Record before state, enable forced change, confirm browser/curl traffic, switch forced clear or unchanged, quit while enabled, and verify restoration. Windows PAC must stop when switching away. | Restore proxy manually from recorded before state and block publication if automatic restore fails. |
| TUN and elevation cleanup | Platform owner | OS routes, DNS, TUN devices, sudo or UAC | Record before state, enable TUN, confirm traffic and DNS, disable, quit, and verify routes, DNS, devices, and elevated processes are cleaned up. | Disable TUN, remove stale routes/devices, kill elevated helpers, and block publication for the affected platform. |
| Runtime supervisor cleanup | Platform owner | Core process tree and logs | Main and pre processes start and stop in expected order; crash or forced stop does not leave orphaned child or elevated processes. | Kill leftover processes, collect logs, and block publication if cleanup is not deterministic. |
| Autostart and hotkeys | Platform owner | Registry, LaunchAgent, desktop autostart file, global hotkey registration | Enable, inspect OS artifact, trigger each hotkey outside the app window, disable, and verify cleanup. | Remove OS autostart artifacts and hotkey registrations manually. Hold publication if cleanup fails. |
| Backup and WebDAV, when credentials exist | Platform owner | Local backup and WebDAV endpoint | Local backup round-trips; WebDAV push and pull restore into a clean profile state with credentials redacted. | Delete test remote artifacts, restore local backup, and mark WebDAV as a known beta gap if credentials were unavailable. |
| Updater smoke | Release engineer and platform owner | Older signed build and beta updater endpoint | Older build detects the new beta, signature validates, update applies, app launches, and version changes. | Re-publish previous `latest.json` or remove beta metadata. Keep direct downloads only if approved. |
| Uninstall cleanup | Platform owner | OS package manager or app removal flow | App removes cleanly, no orphaned process remains, and OS proxy/TUN/autostart/hotkey state is restored. | Remove leftovers manually and block publication if uninstall damages OS state. |

## Pass Criteria

A platform passes release smoke when:

- Signed or approved package installs or launches on a clean account.
- A real server can connect and pass traffic.
- Logs and runtime state are visible.
- System proxy and TUN restore OS state after disconnect, quit, and practical failure paths.
- First-run core acquisition works without default redistribution of restricted core binaries.
- Updater metadata is real and signed for release builds.
- Uninstall or app removal leaves no orphaned process, proxy setting, route, TUN device, autostart entry, or hotkey.

Any release-blocking failure must be linked from the beta release notes or issue tracker before publication continues.
