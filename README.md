# VoyaVPN

VoyaVPN is a greenfield rewrite of v2rayN using Tauri 2, Rust, React,
TypeScript, Tailwind v4, and shadcn/ui foundations.

## Workspace

- `apps/desktop`: `@voya/desktop`, the Tauri desktop application.
- `apps/desktop/src-tauri`: Tauri shell, commands, tray, capabilities, and packaging.
- `apps/desktop/src`: React desktop frontend. Only `apps/desktop/src/ipc` may import `@tauri-apps/api`.
- `apps/mobile`: `@voya/mobile`, the bare React Native app (RN 0.87, no Expo).
- `packages/ui`: source-only shadcn/ui primitives, design tokens, CSS, fonts, and `cn()`.
- `packages/features`: source-only frontend logic and controller hooks shared by the desktop and mobile apps; views stay in each app.
- `packages/i18n`: source-only i18next setup and Voya-maintained locale JSON.
- `packages/utils`: source-only shared formatting, redaction, mounted-ref, and error helpers.
- `crates/voya-contracts`: versioned camelCase IPC and persistence DTOs; the only crate that derives `specta::Type`.
- `crates/voya-core`: pure domain logic and golden-tested sing-box config generation.
- `crates/voya-db`: SQLite repositories and migrations.
- `crates/voya-platform`: OS-specific paths, process, proxy, TUN, and autostart adapters.
- `crates/voya-net`: downloads, updates, subscriptions, Clash API, and ruleset clients.
- `crates/voya-app`: application orchestration, including the product-level proxy runtime backed by the sing-box Clash-compatible API.

The root `package.json` `version` is the release-artifact version read by `vp run release -- artifacts`; keep app, Tauri, and Cargo versions aligned when intentionally bumping releases.

## Setup

The frontend toolchain is [Vite+](https://viteplus.dev) (`vp`): Vite, Vitest,
Oxlint, Oxfmt and type checking behind one CLI, configured in the root
`vite.config.ts`. Install the global CLI once, then the dependencies; `vp`
picks up the pinned pnpm (11.5.0) and Vite+ (catalog) versions itself:

```sh
curl -fsSL https://vite.plus | bash
vp install --frozen-lockfile
```

The full local verification suite also requires the pinned Rust dependency
scanner:

```sh
cargo install cargo-machete --locked --version 0.9.2
```

## Development Commands

Run the full Tauri app in development:

```sh
vp run dev
```

Development runs keep their database, settings, logs, and runtime files in the
`dev/` subdirectory of Tauri's app config directory. On macOS this is
`~/Library/Application Support/app.voyavpn.desktop/dev/`. The first run starts
with fresh settings, so an incompatible database from an installed or older
build does not prevent development startup. Existing data is preserved in the
parent directory; packaged builds (including debug packages) use that parent.

Run the frontend-only Vite dev server:

```sh
vp run dev:web
```

Regenerate Rust-to-TypeScript IPC bindings after command or event type changes:

```sh
vp run generate:bindings
vp run check:bindings
```

## Build Commands

Build the frontend bundle:

```sh
vp run build
```

Build unsigned debug Tauri packages without signing credentials:

```sh
vp run tauri:build --debug
```

On Windows, build and install an unsigned release-profile client plus the
protected TUN service for local testing (the service step opens one UAC prompt):

```powershell
vp run build:windows:local
```

This generates and installs a current-user NSIS artifact, then leaves the
demand-start service stopped until TUN is enabled. The command always targets
the machine's native MSVC Rust triple, regardless of the user's default Rust
toolchain. See
[`docs/release/windows-local-tun-testing.md`](docs/release/windows-local-tun-testing.md).

Build release-profile Tauri packages in a prepared signing environment:

```sh
vp run tauri:build
```

## Test And Verification Commands

Run the complete local CI parity suite:

```sh
vp run verify:local
```

Run the final gate checks individually. `scripts/quality/verify-local.mjs` is the
source of truth for this list and its order; CI's parallel `baseline-fast`,
`baseline-rust` and `baseline-frontend` jobs together run the same steps:

```sh
vp run check:architecture
vp run check:lockfile
vp run check:rust:fmt
vp run check:rust:clippy
vp run check:rust:deps
vp run check:rust:test
vp run check:frontend:static
vp run check:frontend:coverage
vp run check:frontend:bundle
vp run check:frontend:smoke:mock
vp run check:dead-code
vp run check:sing-box
vp run check:bindings
vp run check:i18n
```

`vp run check:frontend:test` runs the same suite without the coverage gate, and
`vp run check:desktop:smoke` (packaged shell through `tauri-driver`) runs in its
own Linux CI job rather than in `verify:local`.

`vp run check:frontend:static` is `vp check`: Oxfmt formatting, Oxlint (with
the type-aware rules) and TypeScript diagnostics for every workspace project.
`vp check --fix` formats and applies the autofixes.

Run one frontend test file (`vp test watch` keeps watching):

```sh
vp test apps/desktop/src/features/profiles/server-table.test.tsx
```

`vp run check:rust:test` runs workspace all-target tests while excluding the Tauri shell library harness, then builds the shell binary test target. The shell library target keeps its lib test harness disabled because shell-level coverage lives in workspace crates and frontend tests; this avoids Windows WebView/Wry loader failures from an otherwise empty harness. Do not use bare `cargo test --workspace --all-targets` on Windows, because Cargo still forces explicitly disabled targets when `--all-targets` is passed.

`vp run check:dead-code` runs both the workspace-wide Knip scan and a strict
production-entry scan. `vp run check:rust:deps` runs cargo-machete against
direct Cargo dependencies.

Linux CI installs Tauri build prerequisites before compiling the Rust workspace. Local Linux machines need the same Tauri system libraries.

## Release Commands

Run the credential-free release workflow equivalent locally:

```sh
vp run verify:local
vp run tauri:build --debug
vp run release -- artifacts --input target/debug/bundle --output dist/release/local --target local-debug --channel beta --allow-empty
vp run release -- updater --input dist/release --out dist/updater/latest.json --target darwin-aarch64,darwin-x86_64,linux-aarch64,linux-x86_64,windows-aarch64,windows-x86_64 --placeholder-signatures
```

The GitHub release workflow is manual-only:

```text
.github/workflows/release.yml
workflow_dispatch inputs: channel, build_profile, dry_run, updater_metadata
```

Generate release-owner evidence scaffolding and validate staged metadata:

```sh
vp run release -- readiness --mode dry-run
vp run release -- check-hosts
```

Production stable publication still requires external signing identities, notarization credentials, updater private keys, CDN publication control, platform smoke machines, and rollback readiness. The release runbooks live under `docs/release/`, and the stable gate report is `docs/verification/stable-release-gate.md`.
