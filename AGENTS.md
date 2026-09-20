# AGENTS.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

VoyaVPN is a greenfield rewrite of v2rayN using Tauri 2 (Rust backend) + React 19 / TypeScript / Tailwind v4 / shadcn/ui (frontend). It generates **sing-box** proxy configs and supervises the core process. There is no v2rayN data migration path — the schema and IPC DTOs are a fresh design, and obsolete v2rayN profile fields (`HeaderType`, `RequestHost`, `Path`, `Extra`, `Ports`, `AlterId`, `Flow`, `Id`, `Security`) must never be introduced.

The package manager is **pnpm 11.5.0** (pinned via Corepack). Rust toolchain is 1.96.0 in CI (workspace MSRV 1.94).

## Monorepo Layout

- `apps/desktop/` — `@voya/desktop`, the Tauri desktop app. It owns `apps/desktop/src`, `apps/desktop/src-tauri`, `apps/desktop/public`, `apps/desktop/e2e`, Vite, Playwright, shadcn, and desktop tsconfigs.
- `apps/web/` — `@voya/web`, placeholder for a future web management surface. Do not copy desktop IPC here; the current backend surface is Tauri IPC only.
- `apps/mobile/` — `@voya/mobile`, the bare React Native app (RN 0.87, no Expo). It does not consume `@voya/ui`, which is DOM/Radix-specific; it consumes `@voya/client`, `@voya/contracts` and `@voya/i18n/native`. Metro needs `watchFolders` plus `unstable_enablePackageExports` for the source-only workspace packages, and hierarchical resolution must stay on — see the reasoning in `apps/mobile/metro.config.js`. Its gates (`check:mobile:test`, `check:mobile:bundle`) are not part of `verify:local`; CI runs them in the `mobile` job, and `check:mobile:swift` on the `mobile-rust` iOS leg. The Rust backend runs in-process through `crates/voya-mobile-ffi` (ADR 0012) and the tunnel runs sing-box inside a platform provider (ADR 0013): an iOS PacketTunnel appex sharing `native/apple/`, and an Android foreground `VpnService` in the app's own process.
- `apps/probe/` — `@voya/probe`, the Cloudflare Worker the self-hosted node's network check calls (`POST /v1/probe`, stateless, connects back only to the caller). Its wire contract is `tests/probe-contract/*.json`, shared with `crates/voya-net`. Deploy per `docs/release/self-host-probe-worker.md`; wrangler is not a workspace dependency.
- `packages/ui/` — `@voya/ui`, source-only shadcn primitives, design tokens, shared CSS, fonts, and `cn()`.
- `packages/contracts/` — `@voya/contracts`, the generated TypeScript view of `crates/voya-contracts`: every IPC DTO, the `VoyaCommands` surface with the result envelope unwrapped, and the three event channels. `src/generated.ts` is derived from `apps/desktop/src/ipc/bindings.ts` by `scripts/quality/contracts-source.mjs`; never edit it, and never hand-write types mirroring backend types anywhere else. The same generator emits `src/commands.ts` (`VOYA_COMMAND_WIRE`, exported as `@voya/contracts/commands`: each command's wire name and argument names, so a non-Tauri transport can rebuild Tauri's named-argument object from a positional call) , `commands.json` (the wire names alone, for a Rust host to check its coverage against) and `events.json` (each channel's wire name and the `kind` values its payload can carry, for the same reason). All four are covered by `check:bindings`.
- `packages/client/` — `@voya/client`, the transport-agnostic domain client shared by every frontend: query keys, backend-code→translation-key maps, the runtime event store, the persisted Zustand stores, the runtime actions (`runtime-action`, `runtime-status`) and the event router that turns the three backend channels into store writes and cache invalidations. It reaches the backend through the `VoyaCommands` registered with `setVoyaCommands`, and reaches the platform through the seams in `src/platform.ts` — storage, colour scheme, clipboard, app visibility and the tunnel's authorization prompt — so it contains no `window` reference. Each app registers its platform in a `platform-boot` module imported first.
- `packages/features/` — `@voya/features`, the frontend logic both apps share: pure helpers (node rows, profile display, subscription usage, import summaries) and controller hooks (`use-home-runtime`, `use-node-list-data`, `use-node-import`, `use-policy-group-runtime`, the form helpers under `forms/`). Views are not shared — the desktop renders shadcn/Radix, mobile renders React Native — so anything that touches the DOM stays in `apps/desktop`. ESLint keeps `packages/client` and `packages/features` off `document`/`window`/`navigator`.
- `packages/i18n/` — `@voya/i18n`, source-only i18next setup and imported locale JSON.
- `packages/utils/` — `@voya/utils`, source-only shared formatting/redaction/error helpers.
- `native/apple/` — the `NEPacketTunnelProvider` Swift sources, shared by the macOS desktop app and the iOS app. The two ship the same provider under different bundle ids and App Groups, so nothing there hardcodes an identifier: the App Group comes from each extension's own `Info.plist` (`VoyaAppGroupIdentifier`). See `native/apple/README.md`.
- `tools/skills/` — tracked agent skills that are not part of the product build: `rollout/` plans multi-phase refactors and generates a resumable `rollout.py` runner (Python; its output goes to the untracked `.agents/rollouts/`).
- `crates/`, `tests/`, `docs/`, and `scripts/` remain rooted at the workspace.
- `docs/` is tracked and holds only what governs or ships with the product: ADRs, release runbooks and legal notices, and design references. `docs/release/THIRD_PARTY_NOTICES.md` is a Tauri bundle resource, so an untracked copy breaks every clean build (`scripts/tauri/core-seeds.test.mjs` guards this). Development-process notes — verification logs, before/after screenshots, usability checks — go in the untracked `.agents/docs/`.

Package scope is always `@voya/*`. The `@/*` alias is desktop-private and resolves to `apps/desktop/src`; shared imports use `@voya/*`.

Version authority: the root `package.json` `version` is the release-artifact version read by `pnpm release -- artifacts`. Keep root, desktop package, Tauri config, and Cargo package versions aligned when doing an intentional version bump; `pnpm run check:architecture` fails when they drift.

## Commands

```sh
pnpm dev                 # Run full Tauri app (backend + frontend) in dev
pnpm dev:web             # Delegates to @voya/desktop frontend-only Vite dev server (127.0.0.1:1420)
pnpm build               # Delegates to @voya/desktop build (tsc -b + vite build)
pnpm --filter @voya/desktop build  # Build only the desktop app
pnpm tauri:build --debug # Unsigned debug Tauri packages (no signing creds needed)

pnpm run verify:local       # Full local verification suite — run this before declaring work done
```

`scripts/quality/verify-local.mjs` is the source of truth for the gate list. CI
splits it across three parallel jobs (`baseline-fast`, `baseline-rust`,
`baseline-frontend`) that together run exactly its steps, each gate once. Run
them individually while iterating:

```sh
pnpm run check:architecture        # Crate/layer boundary rules (see "Architecture gate" below)
pnpm run check:lockfile            # One resolved version per duplication-sensitive package
pnpm run check:rust:fmt            # cargo fmt --all --check
pnpm run check:rust:clippy         # clippy --workspace --all-targets -D warnings
pnpm run check:rust:deps           # cargo-machete 0.9.2; install it locally first
pnpm run check:rust:test           # Workspace tests (see note below) + shell binary test targets
pnpm run check:frontend:typecheck  # pnpm -r run typecheck
pnpm run check:frontend:coverage   # Vitest once + global thresholds + per-module coverage floors
pnpm run check:frontend:lint       # ESLint, uncached: type-aware rules make its per-file cache unsound (`pnpm lint` caches)
pnpm run check:frontend:bundle     # Production build + bundle size budgets
pnpm run check:frontend:smoke:mock # Playwright renderer smoke against the Tauri IPC mock
pnpm run check:dead-code           # Knip workspace scan + strict production scan
pnpm run check:sing-box            # Generated sing-box config acceptance
pnpm run check:bindings            # Fail if generated IPC bindings drift (see IPC below)
pnpm run check:i18n                # Locale key alignment, usage, dynamic keys, visible hardcoded text

# Not part of verify:local; CI runs them in their own jobs:
pnpm run check:desktop:smoke       # Packaged shell through tauri-driver (Linux CI)
pnpm run check:mobile:test         # React Native Jest suite
pnpm run check:mobile:bundle       # Metro bundle for iOS and Android
pnpm run check:mobile:swift        # Parse the iOS app Swift (macOS only)
pnpm run check:frontend:test       # Vitest once, without the coverage gate
pnpm --filter @voya/desktop test --run src/features/profiles/server-table.test.tsx  # Single desktop test file

# Mobile native artifacts. Both write into apps/mobile and are gitignored:
# they are produced from the crate beside them, so a committed copy could only
# be a stale one. Run before an Xcode or Gradle build.
pnpm run native:mobile:rust:ios       # Two iOS slices -> VoyaMobile.xcframework + Swift bindings
pnpm run native:mobile:rust:android   # cargo-ndk -> jniLibs + Kotlin bindings (needs ANDROID_NDK_HOME)
cargo check -p voya-mobile-ffi --target aarch64-apple-ios   # What CI's `mobile-rust` job runs
```

Single Rust test: `cargo test -p voya-core <test_name>` (substitute the owning crate).

`pnpm bench:rust` runs the criterion benchmarks (config generation with 100–3000
nodes in `crates/voya-core/benches/`, subscription import in
`crates/voya-app/benches/`). They are not a gate; run them before and after a
change to config generation or import and compare. `check:rust:test` builds
them in test mode, so they must keep compiling. Startup cost is logged per step
(`startup step` lines in `guiLogs`).

**Do not run bare `cargo test --workspace --all-targets`.** Use `pnpm run check:rust:test` (→ `scripts/quality/rust-tests.mjs`). It runs workspace all-target tests while excluding the Tauri shell lib harness (whose lib test harness is intentionally disabled to avoid Windows WebView/Wry loader failures), then builds the shell binary test target separately; on macOS it also runs the PacketTunnel bridge test (`scripts/native/macos/test-bridge.mjs`). `--all-targets` forces explicitly-disabled targets, breaking Windows.

## Architecture

A Rust workspace of layered crates plus the Tauri desktop shell, React app, and source-only frontend packages. The dependency direction flows: `voya-core` and `voya-contracts` (neither depends on another workspace crate) → `voya-db` / `voya-net` / `voya-platform` → `voya-app` (orchestration) → `apps/desktop/src-tauri` (shell) → `apps/desktop/src` (frontend, via generated IPC). Shared frontend code flows through `packages/*` and must not depend on desktop-private IPC.

### Rust crates (`crates/`)

- **voya-contracts** — Versioned IPC/persistence DTOs shared by the app and the shell, with one canonical camelCase representation and no domain, persistence, network, platform, or Tauri dependencies. It is the **only** crate that derives `specta::Type`: every business and command DTO lives here, and `pnpm run check:architecture` rejects `derive(Type)` in the shell and `specta` in `voya-app`/`voya-core`. Depended on by `voya-db`, `voya-app`, and the shell.
- **voya-core** — Pure, OS-free, deterministic domain logic. Owns models/enums, share-link parsers, routing/DNS logic, and **sing-box config generation** (the generation-related modules are `crates/voya-core/src/config.rs`, `crates/voya-core/src/context.rs`, `crates/voya-core/src/singbox/`; ADR 0003 refers to them as `coregen::`, while the file actually named `coregen.rs` is `crates/voya-app/src/coregen.rs`). Must contain **no** `#[cfg(target_os)]`, OS/Tauri/filesystem/network/process APIs. Clocks, randomness, ports, and platform facts are *injected*.
- **voya-db** — Fresh sqlx SQLite schema, migrations, repositories. It is the **only** typed persistence boundary: tagged `ProfileProtocol`, `ProfileTransport`, TLS settings, and routing rules serialize to SQLite `TEXT` only here.
- **voya-platform** — All OS-specific code: `paths`, `process`, `elevation`, `tun`, `sysproxy`, `autostart`, `coreinfo`, `privilege`. Domain crates reach platform side effects through traits/adapters defined here.
- **voya-net** — HTTP downloads, subscriptions, Clash REST/WebSocket, and ruleset/Geo asset acquisition.
- **voya-mobile-ffi** — The mobile host: the second shell, in the same sense `apps/desktop/src-tauri` is the first. It owns a tokio runtime, calls `AppServices::connect`, injects a rejecting `ProcessRunner` (a phone spawns nothing — the tunnel provider runs the core) and a `NativeTunController` that delegates to the host app, and fans the eight `voya-app` sinks into the three event channels. Its uniffi surface is an envelope, not a model: `invoke(command, args_json) -> json` in the exact wire shape Tauri uses, plus `EventListener` / `TunnelHost` / `ProbeCoreHost` callbacks — see ADR 0012. Two generated files stand in for the types uniffi does not check: `packages/contracts/commands.json` (every command must have a dispatcher, or be on `UNSUPPORTED_ON_MOBILE`, or on `NOT_YET_DISPATCHED` — the three are mutually exclusive and jointly exhaustive) and `packages/contracts/events.json` (the channels and `kind` values its own event enums must carry).
- **voya-app** — Orchestration layer. Managers (one module per subsystem: `runtime`, `supervisor`, `profiles`, `subscriptions`, `routing`, `dns`, `proxy_runtime`, `statistics`, `sysproxy`, `tun`, `elevation`, `updates`, `self_host`, etc.) that combine the domain/db/net/platform crates. `proxy_runtime` exposes product-level connection monitoring and traffic-mode behavior through the sing-box Clash-compatible API. No Tauri wiring here.
- **apps/desktop/src-tauri** — Tauri bootstrap and the *only* backend place that knows about Tauri APIs: command/event registration, `AppState` injection, tray, capabilities, plugins, packaging, lifecycle. `src/lib.rs` `run()` wires everything in `setup()`; IPC lives in `apps/desktop/src-tauri/src/ipc/` (`commands/` holds the `#[tauri::command]` functions split by subsystem, `ipc/window.rs` adds the two window-chrome commands, and the fixed `collect_commands!` list registers every one of them identically in debug and release builds; events live in `events.rs`).

### Frontend (`apps/desktop/src/` + `packages/`)

- **`apps/desktop/src/ipc/` is the only frontend directory allowed to import `@tauri-apps/api` or Tauri plugins.** Features call typed wrappers (`commands.ts`, `tauri-plugins.ts`) and use the single mounted `event-bridge.tsx`, never raw `invoke`/`listen`. This is an architectural rule (ADR 0002) and is lint-enforced. `ipc/register-backend.ts` puts that binding — plus the clipboard, visibility and elevation adapters — behind the `@voya/client` seams; `platform-boot.ts` calls it at startup and `src/test/setup.ts` calls it per test, so a test that mocks `@/ipc/commands` also mocks what shared code sees.
- **`apps/desktop/src/ipc/bindings.ts` is generated** from Rust `specta`/`tauri-specta` — never edit by hand, never hand-write DTOs mirroring backend types. It is regenerated automatically under `pnpm dev` (`run()` exports it when `tauri::is_dev()`, or when `VOYAVPN_EXPORT_BINDINGS` is set); packaged debug builds no longer write it, because the export path is baked in at compile time. After changing any Rust command/event/DTO, run `pnpm generate:bindings` and commit; `pnpm check:bindings` (a CI gate) fails on drift.
- `apps/desktop/src/features/<subsystem>/` — desktop feature UIs (profiles, subscriptions, routing, dns, proxy, settings, logs, qr, updates, home).
- `packages/ui/src/components/` — shared shadcn/ui primitives; `apps/desktop/src/components/app-shell/` — desktop shell. State via Zustand (`apps/desktop/src/stores/`) + TanStack Query.
- `packages/i18n/src/locales/` — Voya-maintained locale JSON; these files are the only translation source.

### IPC event model (three channels)

1. **Invalidation events** — backend changes that invalidate TanStack Query caches (profiles, subscriptions, routing, DNS, settings, proxy runtime).
2. **Transient streams** — live state outside cached queries (log lines, statistics, core state, speedtest, sysproxy/TUN changes). See `TransientStreamEvent` in `events.rs`.
3. **Imperative app events** — shell actions (reload, show/hide, add-via-scan/clipboard, shutdown, set-default-server, etc.).

Command-boundary errors are converted into a typed `AppError` union exposed to TypeScript; crate-internal errors may use local enums.

### Architecture gate (`pnpm run check:architecture`)

`scripts/quality/architecture.mjs` is the first step of `verify:local` and of the
CI `baseline-fast` job. It enforces rules that neither Clippy nor ESLint can express,
so read it before moving code between crates:

- **800 production lines per Rust file.** Lines inside terminal `#[cfg(test)]`
  modules do not count. Split by responsibility when a module outgrows the cap
  (that is what `crates/voya-app/src/subscriptions/update_flow.rs` is).
- **`#[cfg(test)]` items must be terminal test modules.** Everything after the
  first top-level `#[cfg(test)]` must be `#[cfg(test)] mod …` declarations, so
  the production/test split is decidable without compiling.
- **`unsafe` needs a `SAFETY:` comment** within the three preceding lines.
  This covers `unsafe {`, `unsafe impl`, `unsafe extern`, and `unsafe fn`, in
  test code too. Declared test module files (`#[cfg(test)] mod x;`) are
  otherwise exempt from the source rules, which are about shipped code; the
  reason for each exemption is in `architecture.mjs`.
- **`voya-core` must be OS independent and deterministic:** no `#[cfg]` on
  `target_os`/`target_family`/`windows`/`unix`, no `cfg!()` on those, no
  `std::fs`/`std::process`/`std::env`, no `std::net` socket types
  (`TcpStream`/`TcpListener`/`UdpSocket`/`ToSocketAddrs`, matched by name so
  grouped imports are caught; address value types such as `IpAddr` stay
  allowed), no `SystemTime::now`, `Instant::now`, or
  `rand`. Inject clocks, randomness, ports, and platform facts.
- **`voya-app` reaches the network and filesystem through adapters:** no
  `reqwest`, `tokio_tungstenite`, `tokio::net`, `tokio::fs`, `tokio::process`,
  `std::fs`, or `std::process::{Command, Stdio}` (grouped imports such as
  `use std::{fs, io};`, and imports of the `process` module itself such as
  `use std::process;` or `use std::{io, process};`, are matched too;
  `std::process::id()`/`exit` and `std::net` address types stay allowed), and
  no `specta`.
- **The Tauri shell has no tests, no direct domain access, and no DTOs:** no
  `#[cfg(…)]` that enables code under `test` (its lib test harness is disabled),
  no `voya_core::`/`voya_db::`, and no `derive(… Type …)`, plain or inside
  `cfg_attr`, outside `ipc/events.rs`. The standalone binaries under
  `src-tauri/src/bin/` are exempt from these shell rules: they carry their own
  tests, which `check:rust:test` builds as a separate target.
- **Manifest checks** reject a direct, renamed (`x = { package = "…" }`), or
  `[dependencies.…]`-table dependency on `voya-core`/`voya-db` in the shell, and
  on `specta`, `reqwest`, or `tokio-tungstenite` in `voya-app`. They also require
  one release version across the root `package.json`, `apps/desktop/package.json`,
  `apps/desktop/src-tauri/tauri.conf.json`, and every Cargo workspace member
  (all inherit `[workspace.package] version`).
- **Retired v2rayN compatibility stays retired:** no `serde(alias = …)` or
  `rename_all = "PascalCase"` outside `crates/voya-net/src/clash.rs`, no
  `v2rayn://`, and no retired config-compat identifiers. Every `rename_all` and
  `rename_all_fields` in `voya-contracts` must say `camelCase`.

The rule set itself is unit-tested in `scripts/quality/architecture-analyzer.test.mjs`
and `scripts/quality/architecture-rules.test.mjs`.

## Config generation parity (highest-risk area)

Config generation correctness is judged by the **generated sing-box JSON**, not entity snapshots. Golden testing is the parity contract:

- Golden fixtures live in `tests/golden/`: `singbox/` is driven by `matrix.json`; `voya-core` canonicalizes JSON and diffs against this corpus.
- Fixtures must cover ordinary protocols, DNS final/direct detection, TUN, platform pre-socks forwarding, and per-rule outbounds.
- Core acceptance is opt-in: without `VOYA_GOLDEN_ACCEPTANCE` the check is skipped with explicit evidence and JSON golden parity still runs. Once opted in nothing may be skipped — a missing binary panics (set `VOYA_SINGBOX_BIN`), and `pnpm check:sing-box`, which stages the seed and opts in for you, additionally fails unless the libtest summary proves the acceptance test actually ran.
- Raw JSON is allowed only at defined rule-set boundaries — normal profile/DNS/routing/transport/protocol data must be typed.

## Cores and i18n

- The sing-box core seed (GPL-3.0-or-later) is **bundled into every locally built package, debug and dry-run included** — there is no download-on-first-run path in the app. On macOS the PacketTunnel extension's Libbox runs the connection and the seed only backs latency tests while disconnected (launched in place from the signed bundle); while connected, macOS tests go through the running core's Clash API. `postinstall` runs `scripts/core/install-sing-box.mjs`, which fetches the SHA-256-pinned upstream archive at `pnpm install` time into `resources/core-seeds/sing_box/` and copies it into the per-user app-data dir (`VOYAVPN_APP_CONFIG_DIR`, otherwise the OS default). `scripts/tauri/cli.mjs` re-stages the seed for every `tauri build` and injects a generated `bundle.resources` overlay, so the seed ships inside the package; on Windows and Linux the app copies it into app data `bin/` on first run. Skip/force with `VOYAVPN_SKIP_SING_BOX_POSTINSTALL`, `VOYAVPN_FETCH_SING_BOX_ON_INSTALL` (CI postinstall opt-in), `VOYAVPN_FORCE_SING_BOX_FETCH`, or `pnpm core:sing-box:install --force`. Bumping the pin is documented in `docs/release/sing-box-seed-pinning.md`; any package handed to a third party carries GPL redistribution obligations (see `docs/release/THIRD_PARTY_NOTICES.md`).
- Locale files (`packages/i18n/src/locales/*.json`) are maintained directly by Voya and are the sole language source. There are no ResX imports, overlays, or upstream snapshots. `pnpm check:i18n` checks locale alignment and production usage.

## macOS NetworkExtension hygiene

macOS chooses PacketTunnel providers globally by bundle id through PlugInKit,
not by the currently launched VoyaVPN app path. Any copied or launched `.app`
that contains `Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex` can
become the elected provider for `app.voyavpn.desktop.PacketTunnel`.

- After copying, launching, or testing any `.app` with the PacketTunnel appex,
  quit VoyaVPN and run `pnpm native:macos:ne:doctor --fix` (add `--app <path>`
  for a non-`/Applications` bundle and `--dev` for the repo release bundle).
- `pnpm build:mac:local` installs the built app into `/Applications`, strips
  `Contents/PlugIns` from the leftover `target/` copies, and runs the doctor
  itself; see `docs/release/macos-local-tun-testing.md`.
- Fixtures that do not test NetworkExtension behavior must remove
  `Contents/PlugIns` before launching the app.
- Entitlement/provisioning fixtures may keep the production extension id, but
  cleanup is mandatory before deleting the fixture directories.
- Do not add `pnpm native:macos:ne:doctor --fix` to CI or broad verification
  scripts; it intentionally mutates machine-global PlugInKit state.

## Conventions

- Clippy is strict: `unwrap_used`, `dbg_macro`, `todo`, and `all` are warnings, and CI runs clippy with `-D warnings` — avoid `.unwrap()`/`.expect()` outside tests and setup.
- The ADRs indexed in `docs/adr/README.md` are the authoritative design record — consult them before changing crate boundaries or the IPC contract.
- Commit messages in this repo are written in Chinese with `type:` prefixes (feat/fix/refactor/chore/docs); multiple changes are often combined in one message.
- **Mobile styling is Uniwind, not NativeWind.** Both compile Tailwind v4 at build time, but NativeWind v5 styles through `react-native-css`, whose Metro integration is built on `@expo/metro-config` — and that package fails to load without the `expo` SDK beside it, which a bare React Native app does not have. Uniwind detects a non-Expo project and uses Metro's own transform worker. `apps/mobile/global.css` restates the desktop's colour roles (from `packages/ui/src/styles/globals.css`) as Tailwind v4 theme variables in `@variant light` / `@variant dark` blocks, which is Uniwind's theme model; both blocks must declare the same set. The gluestack-ui components under `apps/mobile/src/components/ui` are copied, not depended on — see the README there before adding one.
- **CI OS coverage.** Clippy with `-D warnings` runs on Linux in `baseline-rust` and on macos-15 and windows-2025 in `platform-check`, so `#[cfg(windows)]` / `#[cfg(target_os = "macos")]` code is linted by the strict workspace lints. Rust *tests* run only on Linux on purpose: no `#[cfg(test)]` module in the workspace is OS-gated, so a cross-OS run would re-execute the same suite at triple the wall-clock cost. If you add an OS-gated test module, add `pnpm run check:rust:test` to that matrix in the same change. OS behaviour that only a real machine can exercise is covered at release time by `docs/release/os-smoke-matrix.md`.
- Renderer smoke tests use Playwright with a Tauri IPC mock (`pnpm check:frontend:smoke:mock`); Linux CI separately runs the packaged shell through `tauri-driver` (`pnpm check:desktop:smoke`). Release tooling and runbooks live in `scripts/release/` and `docs/release/`.
