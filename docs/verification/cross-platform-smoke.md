# Cross-Platform Smoke

Batch: `07-05-playwright-tauri-smoke`

## Automated Frontend Smoke

- Command: `pnpm smoke:frontend`
- Harness: Playwright Chromium against `pnpm dev` at `127.0.0.1:1420`.
- Scope: app shell, settings/about/backup/QR dialogs, profile add, VLESS import fixture, fake connect/disconnect, routing profile/rule edit, and DNS save.
- Safety: tests inject a browser-side Tauri IPC mock before app startup. No real proxy, core process, WebDAV endpoint, OS proxy, TUN route, autostart entry, or global hotkey is touched.

## Tauri Driver Status

`tauri-driver` is not a required automated gate in this batch. The local Chromium smoke covers frontend flows non-interactively, while the remaining Tauri-driver value is tied to real WebView/runtime behavior and OS integrations that differ by platform.

Current documented gaps:

| Surface | Why skipped in automation | Manual check |
|---|---|---|
| System proxy modes | Mutates OS proxy settings and restoration state. | Record before state, switch forced set/clear/unchanged, confirm browser/curl behavior, then verify restoration after disconnect and app exit. |
| TUN enable/disable | Requires elevated privileges, routes, DNS, and device cleanup. | Enable TUN with sudo/UAC, confirm all traffic routes, disable, then verify routes/devices/processes are restored. |
| Autostart | Writes registry, LaunchAgent, or desktop autostart files. | Enable, inspect the OS artifact path, reboot or log out/in where feasible, then disable and confirm cleanup. |
| Global hotkeys | Registers desktop-wide shortcuts that can conflict with the user session. | Register each action, trigger it outside the app window, confirm action, then clear bindings. |
| Real connect/core logs | Requires local core binaries and a real redacted server. | Add/import a real profile, connect, confirm logs and traffic, disconnect, and check no orphaned process remains. |
| WebDAV backup | Requires credentials and a reachable remote. | Save credentials, check remote, push backup, pull/restore into a clean state, and remove test artifacts. |
| Package launch | Requires built/signed installers or app bundles. | Install package on each OS, launch, inspect tray/window/config paths, uninstall, and confirm cleanup. |

## Manual OS Matrix

Use `docs/verification/manual-os-smoke.md` as the full evidence template. Minimum checks for this batch:

| OS | Manual smoke target |
|---|---|
| Windows 11 x64 | Launch shell, add/import profile, fake-free real connect, system proxy set/clear, PAC availability, TUN/UAC cleanup, autostart artifact, hotkeys. |
| macOS Apple Silicon | Launch shell, add/import profile, real connect, proxy restore, sudo TUN cleanup, LaunchAgent autostart, hotkeys. |
| Linux x64 | Launch shell, add/import profile, real connect, proxy shell restore, sudo TUN cleanup, desktop autostart file, hotkeys. |

Skipped external checks must record the exact blocker, OS, and follow-up owner before release packaging.
