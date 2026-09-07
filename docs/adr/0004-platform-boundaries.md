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

- `voya-platform::paths`: config, log, temp, application-data, and binary directory resolution.
- `voya-platform::process`: process spawning, termination, permissions, stdout/stderr streaming, and Windows job containment.
- `voya-platform::elevation`: Unix `sudo -S` flow and Windows elevation integration.
- `voya-platform::tun`: TUN setup/teardown helpers and platform-specific preflight.
- `voya-platform::sysproxy` and `voya-platform::pac`: forced clear/change, unchanged, and Windows/macOS PAC behavior.
- `voya-platform::autostart` and `voya-platform::hotkeys`: login startup and global shortcut adapters.
- `src-tauri`: tray, app window lifecycle, capabilities, plugins, sidecar packaging, and user-facing Tauri integration.

Runtime orchestration lives in `voya-app`, but platform side effects are performed through traits/adapters supplied by `voya-platform`.

Security and lifecycle rules:

- Sudo passwords are collected only for TUN/elevated operations, stored in memory only, and zeroized on stop/shutdown.
- Linux and macOS use the same `sudo -S` shape; OS-specific differences belong inside `voya-platform`.
- System proxy and TUN changes must restore on disconnect, app exit, crash restart, and forced disable.
- GPL or AGPL core binaries are redistributed only through an approved packaging path with recorded attribution evidence. See the 2026-09 amendment below for the shape that path actually takes.

Persistence remains a fresh VoyaVPN schema. There is no platform-specific legacy migration code and no obsolete v2rayN columns.

## Consequences

- OS behavior can be smoke-tested independently of core generation.
- Future Linux/macOS/Windows fixes should usually touch `voya-platform` plus tests, not domain crates.
- Tauri-specific code should not leak into `voya-core`, `voya-db`, or config generation.
- Release and packaging work must document manual signing, notarization, elevation, and OS smoke evidence separately from deterministic unit/golden checks.

## Amendment (2026-07): Monorepo Path Contract

The platform boundary remains unchanged after the monorepo migration. Path references in this ADR map as follows:

- Historical `src-tauri` references now map to `apps/desktop/src-tauri`.
- Historical `src` references now map to `apps/desktop/src`.
- Generated IPC bindings now live at `apps/desktop/src/ipc/bindings.ts`.
- Locale JSON now lives at `packages/i18n/src/locales`.

Tauri-specific platform integration still belongs in `apps/desktop/src-tauri`; reusable OS behavior remains in `voya-platform`.

## Amendment (2026-08): Single Storage Layout

VoyaVPN no longer probes for or supports a portable storage mode. `AppPaths` is
constructed from the application-data directory supplied by the Tauri shell,
and all config, database, log, temp, and downloaded-core paths derive from that
directory. No environment variable or executable-adjacent fallback may switch
the persistence root.

## Amendment (2026-09): Core Seed Acquisition Is Build-Time, Not First-Run

The original decision described core acquisition as "download-on-first-run".
That is not what the code does and never was on this branch:

- `scripts/core/install-sing-box.mjs` (the root `postinstall`) fetches the
  SHA-256-pinned upstream sing-box archive at `pnpm install` time into
  `resources/core-seeds/sing_box/` and copies it into the per-user app-data
  directory. It is skipped by `VOYAVPN_SKIP_SING_BOX_POSTINSTALL`, and skipped
  by default on CI unless `VOYAVPN_FETCH_SING_BOX_ON_INSTALL=1`.
- `scripts/tauri/cli.mjs` re-stages the seed for **every** `tauri build`,
  including `--debug` and credential-free dry runs, and injects a generated
  `bundle.resources` overlay from `scripts/tauri/core-seeds.mjs`. Every package
  this repository produces therefore contains the sing-box binary.
- The app has no runtime download path for the core. `voya-platform` copies the
  packaged seed into app data `bin/` on first run; a package built without a
  staged seed simply has no core.

Consequences for the platform boundary are unchanged — seed staging is build
tooling in `scripts/`, and the copy-to-app-data step stays in `voya-platform` —
but every locally built artifact carries GPL-3.0-or-later redistribution
obligations the moment it leaves the build machine. `docs/release/THIRD_PARTY_NOTICES.md`
is bundled into each package and must keep describing the package's real
contents; `docs/release/sing-box-seed-pinning.md` owns the pin-bump procedure.
