# M7 Public Beta Gate Verification

Batch: `08-04-final-regression-evidence`

Run date: 2026-06-01

## Scope

This gate closes Phase 08 packaging and release by proving the final automated regression suite is green and by identifying the remaining public beta work that cannot be completed inside the credential-free runner.

Covered release evidence:

- `docs/release/packaging.md`
- `docs/release/ci-secrets.md`
- `docs/release/runbook.md`
- `docs/release/signing-notarization.md`
- `docs/release/os-smoke-matrix.md`
- `docs/release/rollback.md`
- `docs/verification/manual-os-smoke.md`
- `docs/verification/cross-platform-smoke.md`

## Gate Findings

- The Rust workspace regression suite passed across `voya-core`, `voya-db`, `voya-platform`, `voya-net`, `voya-udptest`, `voya-app`, and `src-tauri`.
- Frontend typecheck, Vitest, ESLint, and generated IPC binding drift checks all passed.
- Tauri packaging configuration is present for macOS, Windows, and Linux, and default installers do not bundle GPL or AGPL proxy core binaries.
- Release CI is configured as a manual `workflow_dispatch` flow with credential-free dry-run defaults for tests, package artifacts, checksums, and placeholder updater metadata.
- Release runbooks document the manual signing, notarization, updater key, OS smoke, rollback, and publication checkpoints.

## Fixes Applied

- Added this M7 public beta gate report.
- Updated `README.md` with setup, development, build, test, verification, and release commands.
- No code fixes were required for the final automated checks.

## Automated Evidence

Final local results:

```sh
cargo test --workspace --all-targets
```

PASS. Workspace tests completed successfully across Rust crates and `src-tauri`.

```sh
pnpm typecheck
```

PASS. TypeScript project references compiled with `tsc -b --pretty false`.

```sh
pnpm test -- --run
```

PASS. Vitest completed 4 test files and 18 tests.

```sh
pnpm lint
```

PASS. ESLint completed without findings.

```sh
pnpm bindings:check
```

PASS. `scripts/bindings.mjs --check` regenerated bindings in a temporary path and reported that `src/ipc/bindings.ts` is up to date.

```sh
test -f docs/verification/m7-public-beta-gate.md
```

PASS. This report is present.

## Package And Release Evidence

Credential-free package validation is documented in `docs/release/packaging.md`. The current release path is:

```sh
pnpm run verify:ci
pnpm tauri:build --debug
node scripts/release-artifacts.mjs --input target/debug/bundle --output dist/release/local --target local-debug --channel beta --allow-empty
node scripts/release-updater-metadata.mjs --input dist/release --out dist/updater/latest.json --target darwin-x86_64,windows-x86_64,linux-x86_64 --placeholder-signatures
```

The generated artifacts from this path are for validation only. Unsigned debug packages and placeholder updater signatures are not publishable public beta artifacts.

## Deferred External Checks

The remaining release prerequisites are intentionally external to this automated batch:

| Check | Reason not completed by runner | Follow-up |
| --- | --- | --- |
| macOS Developer ID signing and notarization | Requires Apple account credentials, Developer ID identity, and notarization access. | Follow `docs/release/signing-notarization.md`, then run macOS smoke from `docs/release/os-smoke-matrix.md`. |
| Windows Authenticode signing | Requires certificate, signing service, hardware token, or secure CI secret import. | Sign NSIS/MSI artifacts and record Windows smoke evidence before publication. |
| Real updater metadata | Requires approved updater keypair, signed updater payloads, and beta update hosting. | Replace the updater public key placeholder, supply private key only through secure release systems, and generate non-placeholder `latest.json`. |
| Real OS smoke matrix | Requires clean Windows, macOS, and Linux machines with permission to mutate proxy, TUN, autostart, and hotkey state. | Execute `docs/release/os-smoke-matrix.md` and attach evidence using `docs/verification/manual-os-smoke.md`. |
| Real connect and traffic flow | Requires private server or subscription credentials and installed or first-run-downloaded third-party core binaries. | Run the manual core discovery and connection smoke with redacted server evidence. |
| WebDAV live backup | Requires a reachable test WebDAV endpoint and credentials. | Run the manual WebDAV smoke or document it as a beta gap with owner and follow-up. |
| Public beta publication | Requires release owner approval, artifact hosting, update hosting, checksums, and rollback readiness. | Publish only after all signing, updater, OS smoke, and rollback checkpoints pass. |

## Exit Status

Automated M7 gate status: PASS.

Remaining work is limited to external credentials, OS machines, real third-party core/server inputs, and publication actions.
