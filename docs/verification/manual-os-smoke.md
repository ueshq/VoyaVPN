# Manual OS Smoke

Batch: `00-03-verification-scaffold-plan`

These checks require real Windows, Linux, or macOS machines because they mutate OS proxy state, routes, TUN devices, autostart entries, hotkeys, process trees, package trust state, or signing/notarization surfaces. They are separate from the deterministic local gates in `docs/verification/strategy.md`.

## Evidence Record

For each run, capture:

- Date and operator
- VoyaVPN commit or build identifier
- OS name, version, architecture, and desktop environment where relevant
- Install mode: `pnpm tauri dev`, unsigned package, signed package, or release build
- Core binaries used, path, version, and acquisition method
- Test server or subscription source used, with secrets redacted
- Commands run and visible app actions taken
- Logs, screenshots, or terminal output that prove pass/fail
- Skipped checks with concrete reason and follow-up
- Before/after state for proxy, routes, TUN devices, autostart, and running processes when those surfaces are tested

External core binaries are not required by the baseline docs batch. Later manual runs should use locally installed or first-run-downloaded binaries. Do not treat absent core binaries as a packaging failure unless the check specifically targets core acquisition.

## Machine Matrix

Minimum release evidence should cover:

| OS | Required coverage |
|---|---|
| Windows 11 x64 | install/launch, connect, system proxy, PAC, TUN/UAC, process cleanup, autostart, hotkeys, uninstall |
| Windows 10 x64 | connect, system proxy restore, package launch, updater smoke if supported |
| macOS Apple Silicon | signed/notarized launch, connect, sudo TUN, proxy restore, autostart, hotkeys |
| macOS Intel, if supported | package launch, connect, proxy restore |
| Linux x64 | AppImage or package launch, connect, sudo TUN, proxy shell restore, autostart, hotkeys |

Add arm64 Linux, rpm-based Linux, or other distro evidence before claiming support for those targets.

## Smoke Flow

Run the checks that apply to the current implementation phase.

### Launch And Shell

- Start the app from the intended mode.
- Confirm the main window opens, tray appears where supported, logs screen is usable, and no startup errors appear.
- Confirm generated IPC-backed commands used by the shell respond correctly.
- Close and reopen the app; verify no stale process remains.

### Core Discovery And Connection

- Confirm the app finds or downloads the selected core without bundling GPL/AGPL binaries by default.
- Add or import a real server.
- Connect through Xray and sing-box when both are supported by the phase.
- Confirm logs stream, status changes, local SOCKS/mixed inbound accepts traffic, and a browser or curl request exits through the proxy.
- Disconnect and verify the core process exits.

### System Proxy

- Record the OS proxy state before the test.
- Enable forced-change system proxy and confirm browser traffic routes through VoyaVPN.
- Switch to forced-clear or unchanged and confirm the OS proxy state is restored as expected.
- On Windows, test PAC mode and confirm PAC is stopped when switching away.
- Quit the app while proxy is enabled and confirm restoration on exit.
- Simulate or force a core crash when practical and confirm proxy restoration or safe state.

### TUN And Elevation

- Record routes, DNS state, and TUN devices before the test.
- Enable TUN.
- On Linux/macOS, confirm sudo password is requested only for the elevation flow and is not persisted.
- On Windows, confirm the UAC or driver cleanup flow appears as expected.
- Confirm all traffic routes through the proxy and DNS still resolves.
- Disable TUN and confirm routes, DNS, devices, and elevated processes are cleaned up.
- Quit while TUN is enabled and confirm cleanup.

### Runtime Supervisor

- Connect a profile that requires pre-socks or chained generation when available.
- Confirm main and pre processes start in the expected order and stop cleanly.
- Force-stop the core process and confirm crash handling, log output, and restart/cleanup behavior match the current phase contract.
- Verify no orphaned elevated process, child process, or Windows job-owned process remains.

### Profiles, Imports, And Subscriptions

- Add one profile manually for each implemented protocol family.
- Import share links and a subscription with duplicate entries.
- Confirm deduplication, filter behavior, active server selection, sorting, and profile edits persist after restart.
- Export or copy a share link where supported and re-import it into a clean profile group.

### Routing, DNS, Groups, And Chains

- Select routing and DNS settings that are covered by golden fixtures.
- Reconnect and confirm traffic follows direct, block, and proxy rules.
- Test policy groups and proxy chains with at least two children and one mixed chain/group case.
- Confirm generated Xray and sing-box behavior matches the fixture expectation from the same case.

### Clash, Speedtest, Updates, And Backup

- Open Clash proxies and connections screens against a running Clash-compatible core.
- Change rule mode and verify the app uses the configured API behavior.
- Run each implemented speedtest mode and confirm results update the server table.
- Check app/core/geo/ruleset update flows without redistributing restricted binaries by default.
- Create a local backup, restore into a clean profile state, and repeat with WebDAV when credentials are available.

### Packaging And Release

- Install the platform package from a clean user account.
- Confirm app launch, config directory creation, tray, first-run core acquisition, and uninstall behavior.
- Windows: verify Authenticode signature and installer trust prompts.
- macOS: verify Developer ID signature, notarization, quarantine behavior, and DMG/app launch.
- Linux: verify package metadata, desktop file, icons, AppImage or package permissions, and uninstall cleanup.
- Confirm updater metadata and signatures only on release builds with the correct credentials.

## Pass Criteria

A manual smoke run passes when:

- The app launches and exits cleanly.
- A real server can connect and pass traffic through the expected local inbound.
- Logs, core state, and live traffic or stats are visible when implemented.
- System proxy and TUN restore OS state after disconnect, exit, and failure paths.
- No orphaned core, elevated helper, TUN device, route, proxy setting, autostart entry, or hotkey remains after cleanup.
- Packaging installs, launches, and uninstalls on the target OS without bundling restricted core binaries by default.

Any failure should be linked to a reproducible issue with OS details, logs, and before/after state.
