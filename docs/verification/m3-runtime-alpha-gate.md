# M3 Runtime Alpha Gate

Batch: `04-06-alpha-phase-gate`

Run date: 2026-06-01

## Scope

This gate stabilizes the Phase 04 runtime alpha surface after connect/logs, supervisor/elevation, system proxy/tray, and statistics batches.

Covered automated surfaces:

- Runtime connect, restart, disconnect, status, generated config writing, and cleanup.
- Actor-owned supervisor lifecycle, pre-core handling, Unix sudo password storage, Windows job/TUN cleanup seams, and teardown order.
- Core stdout/stderr log forwarding to transient `logLine` events and the Logs tab.
- System proxy mode planning, Windows-only PAC gating, forced restore on disconnect/exit, and tray proxy actions.
- Xray and sing-box statistics readers, active-server traffic persistence, date rollover, and frontend speed/traffic columns.
- Generated IPC bindings and frontend typed-wrapper usage.

## Automated Evidence

Final local results:

```sh
cargo test --workspace --all-targets
```

PASS. Workspace tests completed successfully across all Rust crates and `src-tauri`.

```sh
pnpm typecheck
```

PASS. TypeScript project references compiled with `tsc -b --pretty false`.

```sh
pnpm test -- --run
```

PASS. Vitest completed 2 test files and 8 tests. A concurrent exploratory run timed out while Cargo was still compiling; the required standalone command passes.

```sh
pnpm lint
```

PASS. ESLint completed without findings.

```sh
pnpm bindings:check
```

PASS. `scripts/bindings.mjs --check` regenerated bindings in a temporary path and reported that `src/ipc/bindings.ts` is up to date.

```sh
test -f docs/verification/m3-runtime-alpha-gate.md
```

PASS. This report is present.

## Manual Real-Server Smoke

These steps require a valid server, a downloaded core binary, and permission to mutate local proxy settings. Do not store credentials in the repository or test logs.

1. Start from a clean app process and choose the core to smoke:
   - Xray: place `xray` or `xray.exe` under the Tauri app config directory at `bin/xray/`.
   - sing-box: place `sing-box`, `sing-box-client`, or the Windows `.exe` equivalent under `bin/sing_box/`.
   - In the current app, runtime state is created below the Tauri app config directory for identifier `app.voyavpn.desktop`; the app creates `guiConfigs`, `bin`, `binConfigs`, `guiLogs`, and `guiTemps` there.
2. Run `pnpm tauri dev`.
3. Add or import one real profile. Set it active in the Profiles table.
4. Confirm the active profile uses the intended core. Leave TUN disabled for the first smoke unless this is a privileged TUN run.
5. Click Connect in the status bar or use tray `Connect`.
6. Confirm the status bar changes to connected, shows a main PID, and shows the expected core label.
7. Confirm `binConfigs/config.json` exists. If the active profile uses a pre-socks context, confirm `binConfigs/configPre.json` exists and the status bar or runtime status exposes a pre PID.
8. Open the Logs tab and confirm process lines stream with `[main]` or `[pre]` prefixes. Startup errors must be visible here.
9. Send traffic through the local inbound. The default SOCKS port is `127.0.0.1:10808`; if the config was changed, use `Inbound[0].LocalPort` from `guiConfigs/guiNConfig.json`.
10. Verify traffic flow with a non-sensitive endpoint, for example `curl --socks5-hostname 127.0.0.1:10808 https://example.com` or a browser configured to the same local SOCKS port.
11. Enable system proxy mode `Set` from the status bar or tray. Confirm browser traffic routes without manual proxy configuration.
12. While traffic is flowing, confirm the status bar upload/download speed becomes non-zero and the active profile traffic columns update after the coalesced statistics interval.
13. Switch system proxy mode to `Clear`, then back to `Set`, and confirm the UI effective mode follows the requested mode. On Windows, repeat with PAC if PAC is available in the UI.
14. Click Disconnect or use tray `Disconnect`.
15. Confirm status is disconnected, upload/download speed returns to zero, `binConfigs/config.json` and `binConfigs/configPre.json` are removed, and the core process PIDs no longer exist.
16. Exit the app from the tray Quit item. Confirm system proxy settings are restored or cleared and no core, pre-core, PAC, or elevated helper process remains.

## Privileged TUN Smoke

Run only on a disposable or approved host because TUN changes routes and may require elevation.

1. Submit sudo/UAC credentials through the runtime sudo prompt before enabling TUN on Unix platforms.
2. Enable TUN, connect the same real profile, and confirm the app either starts the elevated core path or surfaces a missing-password error without hanging.
3. Verify all traffic routes through the proxy, not only the browser.
4. Disconnect and exit. Confirm routes, TUN devices, system proxy state, and elevated helper processes are cleaned up.

## Deferred External Checks

- Real server, proxy, and TUN smoke were not executed in the automated gate because they require private credentials, downloaded third-party core binaries, and host-level network/proxy mutation.
- Core binaries remain external; this gate does not vendor or redistribute GPL/AGPL core artifacts.
- Cross-OS proxy readback and TUN cleanup still need manual evidence on Windows, Linux, and macOS smoke machines.
