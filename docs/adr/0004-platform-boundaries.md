# ADR 0004: Platform Boundaries

Status: Accepted

Date: 2026-05-31

## Context

v2rayN spreads platform behavior across managers, helpers, UI projects, and OS-specific proxy/process code. VoyaVPN must support Windows, Linux, and macOS from the first scaffold while keeping domain logic testable.

Reference evidence:

- system proxy: `ServiceLib/Handler/SysProxy/**`
- process and Windows jobs: `ServiceLib/Services/ProcessService.cs`, `ServiceLib/Services/WindowsJobService.cs`
- core runtime/elevation: `ServiceLib/Manager/CoreManager.cs`, `ServiceLib/Manager/CoreAdminManager.cs`
- TUN and OS utilities: `ServiceLib/Common/WindowsUtils.cs`, `ServiceLib/Sample/kill_as_sudo_linux_sh`, `ServiceLib/Sample/kill_as_sudo_osx_sh`
- UI-triggered platform workflows: `ServiceLib/Events/AppEvents.cs`, WPF and Avalonia views

## Decision

All OS-specific implementation lives in `voya-platform` or the Tauri shell where Tauri owns the API. `voya-core` must remain free of `#[cfg(target_os)]`, OS APIs, process launching, registry access, shell scripts, Tauri APIs, and filesystem path discovery.

Boundary ownership:

- `voya-platform::paths`: config, log, temp, portable-mode, and binary directory resolution.
- `voya-platform::process`: process spawning, termination, permissions, stdout/stderr streaming, and Windows job containment.
- `voya-platform::elevation`: Unix `sudo -S` flow and Windows elevation integration.
- `voya-platform::tun`: TUN setup/teardown helpers and platform-specific preflight.
- `voya-platform::sysproxy` and `voya-platform::pac`: forced clear/change, unchanged, and Windows-only PAC behavior.
- `voya-platform::autostart` and `voya-platform::hotkeys`: login startup and global shortcut adapters.
- `src-tauri`: tray, app window lifecycle, capabilities, plugins, sidecar packaging, and user-facing Tauri integration.

Runtime orchestration lives in `voya-app`, but platform side effects are performed through traits/adapters supplied by `voya-platform`.

Security and lifecycle rules:

- Sudo passwords are collected only for TUN/elevated operations, stored in memory only, and zeroized on stop/shutdown.
- Linux and macOS use the same `sudo -S` shape; OS-specific differences belong inside `voya-platform`.
- System proxy and TUN changes must restore on disconnect, app exit, crash restart, and forced disable.
- GPL or AGPL core binaries are not redistributed in installers by default; core acquisition is download-on-first-run or an explicitly approved packaging path.

Persistence remains a fresh VoyaVPN schema. There is no platform-specific legacy migration code and no obsolete v2rayN columns.

## Consequences

- OS behavior can be smoke-tested independently of core generation.
- Future Linux/macOS/Windows fixes should usually touch `voya-platform` plus tests, not domain crates.
- Tauri-specific code should not leak into `voya-core`, `voya-db`, or config generation.
- Release and packaging work must document manual signing, notarization, elevation, and OS smoke evidence separately from deterministic unit/golden checks.
