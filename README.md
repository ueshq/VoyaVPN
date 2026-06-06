# VoyaVPN

VoyaVPN is a greenfield rewrite of v2rayN using Tauri 2, Rust, React,
TypeScript, Tailwind v4, and shadcn/ui foundations.

## Workspace

- `crates/voya-core`: pure domain logic and future config generation.
- `crates/voya-db`: SQLite repositories and migrations.
- `crates/voya-platform`: OS-specific paths, process, proxy, TUN, and hotkey adapters.
- `crates/voya-net`: downloads, updates, subscriptions, Clash API, and WebDAV clients.
- `crates/voya-udptest`: UDP tester support.
- `crates/voya-app`: application orchestration across the domain crates.
- `src-tauri`: Tauri shell, commands, tray, capabilities, and packaging.
- `src`: React frontend. Only `src/ipc` may import `@tauri-apps/api`.

## Setup

Install the pinned frontend toolchain and dependencies:

```sh
corepack enable
corepack prepare pnpm@11.5.0 --activate
pnpm install --frozen-lockfile
```

## Development Commands

Run the frontend-only Vite dev server:

```sh
pnpm dev
```

Run the full Tauri app in development:

```sh
pnpm tauri:dev
```

Regenerate Rust-to-TypeScript IPC bindings after command or event type changes:

```sh
pnpm bindings
pnpm bindings:check
```

## Build Commands

Build the frontend bundle:

```sh
pnpm build
```

Build unsigned debug Tauri packages without signing credentials:

```sh
pnpm tauri:build --debug
```

Build release-profile Tauri packages in a prepared signing environment:

```sh
pnpm tauri:build
```

## Test And Verification Commands

Run the complete local CI parity suite:

```sh
pnpm run verify:ci
```

Run the final gate checks individually:

```sh
cargo test --workspace --all-targets
pnpm typecheck
pnpm test --run
pnpm lint
pnpm bindings:check
```

Linux CI installs Tauri build prerequisites before compiling the Rust workspace. Local Linux machines need the same Tauri system libraries.

## Release Commands

Run the credential-free release workflow equivalent locally:

```sh
pnpm run verify:ci
pnpm tauri:build --debug
node scripts/release-artifacts.mjs --input target/debug/bundle --output dist/release/local --target local-debug --channel beta --allow-empty
node scripts/release-updater-metadata.mjs --input dist/release --out dist/updater/latest.json --target darwin-aarch64,darwin-x86_64,linux-aarch64,linux-x86_64,windows-aarch64,windows-x86_64 --placeholder-signatures
```

The GitHub release workflow is manual-only:

```text
.github/workflows/release.yml
workflow_dispatch inputs: channel, build_profile, dry_run, updater_metadata
```

Production stable publication still requires external signing identities, notarization credentials, updater private keys, CDN publication control, diagnostics approval, platform smoke machines, and rollback readiness. The release runbooks live under `docs/release/`, and the stable gate report is `docs/verification/stable-release-gate.md`.
