# Autostart, Hotkeys, QR Verification

Batch: `07-02-autostart-hotkeys-qr`

## Implemented Surface

- Autostart is implemented through `voya-platform::autostart` with pure plans plus an adapter-backed executor.
- App commands expose autostart status and enable or disable through generated IPC.
- Global hotkeys are normalized to the five `EGlobalHotkey` actions:
  - `ShowForm`
  - `SystemProxyClear`
  - `SystemProxySet`
  - `SystemProxyUnchanged`
  - `SystemProxyPac`
- Tauri registers persisted hotkeys with `tauri-plugin-global-shortcut` during startup and after settings saves.
- QR generation is backend-owned through `voya-app::qr` and returns SVG image data over typed IPC.
- QR scan remains a frontend/platform path. The React dialog uses the WebView `BarcodeDetector` API when available and imports decoded text through the existing profile import command.

## Autostart Artifacts

| OS | Artifact | Notes |
| --- | --- | --- |
| Windows | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value named `VoyaVPN_<path-hash>` | Uses the per-user Run registry key. The v2rayN administrator Task Scheduler path remains a packaging/manual-smoke follow-up if elevated startup is required. |
| Linux | `~/.config/autostart/VoyaVPN.desktop` | Desktop entry mirrors the reference autostart template with VoyaVPN naming and the current executable path. |
| macOS | `~/Library/LaunchAgents/VoyaVPN-LaunchAgent.plist` | LaunchAgent is unloaded before rewrite and loaded after enable. |

## Automated Evidence

- `cargo test -p voya-platform autostart --all-targets`
- `cargo test -p voya-app hotkey --all-targets`
- `pnpm typecheck`
- `pnpm test -- --run`
- `test -f docs/verification/autostart-hotkeys-qr.md`

## External Checks

Real OS mutation was not executed in this batch. Registry writes, LaunchAgent loading, `.desktop` startup behavior, and global hotkey capture require Windows, macOS, and Linux desktop smoke machines and are covered by `docs/verification/manual-os-smoke.md`.
