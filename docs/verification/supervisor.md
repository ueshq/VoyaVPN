# Supervisor And Elevation Verification

Batch: `04-02-supervisor-elevation`

## Scope

- Added `voya-platform::process` for spawn, one-shot commands, stop, generated scripts, fake-runner seams, and Windows job abstraction.
- Added `voya-platform::elevation` for in-memory `Zeroizing` sudo password storage, Unix `sudo -S` run wrapping, and OS-specific sudo kill script selection.
- Added `voya-platform::tun` for Windows stale TUN cleanup abstraction and deterministic no-op tests.
- Added `voya-app::supervisor` actor with serialized start, stop, restart, crash restart, main/pre lifecycle, job assignment, TUN cleanup ordering, and sudo teardown.
- Added `voya-app::sudo` request-response collection primitive and Tauri commands for begin, submit, clear, and status.

## Reference Points

- v2rayN lifecycle: `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib/Manager/CoreManager.cs`
- v2rayN sudo wrapper: `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib/Manager/CoreAdminManager.cs`
- v2rayN process wrapper: `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib/Services/ProcessService.cs`
- v2rayN Windows job object: `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib/Services/WindowsJobService.cs`
- v2rayN TUN cleanup: `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib/Common/WindowsUtils.cs`

## Deterministic Evidence

- `cargo test -p voya-platform process --all-targets`
- `cargo test -p voya-app supervisor --all-targets`
- `pnpm bindings`
- `pnpm bindings:check`

The tests use fake process runners and fake job/TUN adapters. Real OS elevation, process tree teardown, UAC, and TUN device mutation are intentionally left for the manual OS smoke matrix because they require host privileges and platform-specific devices.
