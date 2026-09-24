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
- `voya-platform::elevation`: Unix `sudo -n` launcher flow and Windows elevation integration (see the 2026-09 elevation amendment below).
- `voya-platform::tun`: TUN setup/teardown helpers and platform-specific preflight.
- `voya-platform::sysproxy`: forced clear/change and unchanged behavior. Local PAC hosting and custom system-proxy scripts were retired on 2026-09-13 (see ADR 0007).
- `voya-platform::autostart`: login startup adapters.
- `src-tauri`: tray, app window lifecycle, capabilities, plugins, sidecar packaging, and user-facing Tauri integration.

Runtime orchestration lives in `voya-app`, but platform side effects are performed through traits/adapters supplied by `voya-platform`.

Security and lifecycle rules:

- No sudo password is collected or stored. A one-time native authorization installs a root-owned launcher that elevated TUN cores start and stop through with `sudo -n` (see the 2026-09 elevation amendment below).
- Linux and macOS use the same `sudo -n` launcher shape; OS-specific differences belong inside `voya-platform`.
- System proxy and TUN changes must restore on disconnect, app exit, crash restart, and forced disable. [ADR 0007](0007-macos-manual-system-proxy.md) amends system proxy restoration on macOS to manual configuration and read-only inspection; automatic restoration remains for Windows/Linux.
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

### Follow-up (2026-09-18): macOS Packages Carry No Seed

Since macOS became NetworkExtension-only, the PacketTunnel extension (which
statically links Libbox, i.e. sing-box) is the only core that runs there, and
the standalone seed was a second ~50 MB copy of the same Go runtime used only to
launch speedtest probe cores. macOS packages now ship no seed:
`scripts/tauri/core-seeds.mjs` emits no seed overlay for darwin targets, the
runtime hands the PacketTunnel a config without resolving any executable, and
the speedtest measures nodes through the running core's Clash API against
unrouted per-node probe outbounds (`voya_core::latency_probe_tag`). Latency
tests on macOS therefore need an active connection. The pinned darwin archives
stay in the installer for developer tooling (`pnpm check:sing-box`); they are
never bundled. The GPL obligations are unchanged, since Libbox is sing-box.

### Follow-up (2026-09-18, later): The macOS Seed Returns for Disconnected Speedtests

Greying out every latency test until the user connects was the wrong trade for
~50 MB, so macOS packages bundle the seed again, used only for speedtests:

- The connection still runs only in the PacketTunnel: `RuntimeManager` hands it
  a config and never launches the seed.
- The speedtest decides per run. While the supervisor reports a connected core,
  it measures through that core's Clash API as above: a probe core's own
  traffic would otherwise enter the tunnel and measure the node through the
  VPN. Otherwise it launches probe cores exactly as Windows and Linux do.
- macOS launches the seed where it lies inside the signed bundle
  (`discover_packaged_seed_executable`); copying it into app data would lose
  the code-signing context. `sign-app.mjs` signs it and `verify-tunnel.mjs`
  requires it.

### Follow-up (2026-09-24): Default Rule Sets Ship With the Seed

The default routing profile (ADR 0009) names `geosite-cn`, `geoip-cn` and
`geosite-private`. Until a rule-library update ran, the generated config
pointed at remote rule sets that sing-box fetches through the proxy on first
start, so a node that could not reach GitHub left the China and LAN rules
without data. Packages now bundle those three files next to the seed
(`core-seeds/rule_sets/`, pinned in `scripts/core/rule-sets-installer.mjs`),
and every desktop OS, macOS included, copies any missing one into app data
`bin/srss/` at startup (`voya_app::updates::install_seed_rule_sets`), where the
generator already looks for local rule sets. Existing files are never
replaced. The mobile apps still fetch them remotely: neither has a bundled
resource path yet.

## Amendment (2026-09): Passwordless Unix Elevation

The original decision described a `sudo -S` flow that piped a collected admin
password to sudo and zeroized it on stop. The code does not work that way:

- `voya-platform::privilege` asks the OS once (macOS `osascript ... with
  administrator privileges`, Linux `pkexec`) to install a fixed-path,
  root-owned launcher plus a `NOPASSWD` sudoers drop-in that authorizes only
  that launcher. The launcher's `uninstall` verb removes both on exit.
- `voya-platform::elevation` runs the elevated core and its kill as
  `sudo -n -- <launcher> run|kill ...`, so no process ever receives a password
  on stdin. The process layer has no stdin path, and the `zeroize` dependency
  that guarded the piped password is gone.
- The residual risk is documented in `privilege.rs`: the core binaries live in
  a user-writable directory, so the launcher's path checks contain, but do not
  remove, privilege escalation by an attacker already running as the same user.
