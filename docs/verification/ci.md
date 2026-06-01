# CI Baseline

## Scope

The baseline workflow lives at `.github/workflows/ci.yml` and runs on pull
requests plus pushes to `main`. It validates the foundation without requiring
signing credentials, packaged installers, notarization, or bundled core
binaries.

## Local Parity

Run the same sequence locally:

```sh
pnpm install --frozen-lockfile
pnpm run verify:ci
```

`pnpm run verify:ci` calls the package scripts that CI uses:

```sh
pnpm run check:rust:fmt
pnpm run check:rust:test
pnpm run check:frontend:typecheck
pnpm run check:frontend:test
pnpm run check:frontend:lint
pnpm run check:bindings
```

The commands are non-interactive. The generated binding drift check is a
dedicated CI step and fails when `src/ipc/bindings.ts` differs from the Rust
Specta export.

## CI Environment

- Rust: stable toolchain, with workspace `rust-version` set in `Cargo.toml`.
- Node: 22.
- pnpm: 11.5.0.
- Ubuntu runner: installs Tauri Linux prerequisites before compiling Rust.

## Deferred External Checks

- Tauri packaging is not part of this baseline because release installers later
  require platform-specific signing or notarization setup.
- Xray, sing-box, and other core binary acceptance checks are not part of this
  baseline because binaries are downloaded or discovered in later config-gen and
  runtime batches.
- Real Windows, Linux, and macOS smoke checks remain manual OS evidence until
  platform-specific behavior exists.

## Latest Local Evidence

Verified on 2026-05-31:

```text
pnpm run verify:ci
CI baseline checks passed.
```
