# TUN Mode Polish Verification

Batch: `05-03-tun-mode-polish`

## Implemented Scope

- Added `voya-platform::tun` preflight reporting for Windows, Linux, macOS, and unsupported platforms.
- Added `voya-app::tun::TunManager` for status and enable/disable decisions backed by the stored in-memory sudo password.
- Added `tun_status` and `set_tun_enabled` IPC commands with generated TypeScript bindings.
- Added a status-bar TUN toggle. On Unix, enabling TUN starts sudo collection once before persisting `TunModeItem.EnableTun`.
- Kept the process paths distinct:
  - sing-box and mihomo TUN starts use Unix `sudo -S` wrapping.
  - Xray is not sudo-wrapped by the supervisor; its TUN path remains the Xray config-generated `tun` inbound.
- Tightened supervisor partial-start cleanup so an elevated main process is sudo-killed if a later pre-core spawn fails.

## Automated Coverage

- `voya-platform::tun`:
  - Unix `allow_enable_tun` is false until a non-empty sudo password exists.
  - Windows preflight reports the stale Wintun cleanup device names `wintunsingbox_tun` and `xray_tun`.
  - Route restoration notes are surfaced for manual smoke evidence.
- `voya-app::tun`:
  - Unix enable fails with a sudo-required error until the in-memory password exists.
  - Disable is allowed without a sudo password.
  - Windows status reports manual driver smoke and cleanup device names.
- `voya-app::supervisor` fakes:
  - sing-box and mihomo are sudo-wrapped under TUN on Unix; Xray is not.
  - Partial start failure sudo-kills an already-started elevated main process before returning.
  - Windows TUN cleanup still runs before process start and job assignment.

## Restore-On-Disconnect Notes

The automated fake tests assert teardown order and partial-start cleanup, not host route mutation. Runtime disconnect stops elevated TUN cores through sudo kill before normal process stop, so core-owned routes and interfaces can unwind on process exit. Windows relies on job containment and pre-start stale-device cleanup; route/device restoration must be checked on real Windows hosts.

The expected operational rule is: after disconnect, restart failure, app exit, or TUN disable/restart, no elevated helper, core child process, TUN device, or route owned by VoyaVPN should remain.

## Manual OS Smoke

Real TUN and driver checks were not executed in this automated batch because they require host-level network route mutation, sudo/UAC credentials, installed third-party core binaries, and platform TUN drivers. Follow-up smoke must record before/after state for routes, DNS, TUN devices, and process trees on:

- Windows: UAC prompt, Wintun device cleanup, job-owned process cleanup, route restoration.
- Linux: sudo collection once at enable, sing-box/mihomo elevated launch, route restoration after disconnect.
- macOS: same `sudo -S` path as Linux, macOS route/interface restoration, no orphan elevated process.

## Verification Commands

- `cargo test -p voya-platform tun --all-targets`
- `cargo test -p voya-app tun --all-targets`
- `pnpm typecheck`
- `test -f docs/verification/tun.md`
