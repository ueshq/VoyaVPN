# M1 Profile Gate Verification

Batch: `02-05-profile-phase-gate`

Run date: 2026-05-31

## Scope

This gate stabilizes the Phase 02 profile and import surface before config generation begins:

- Persisted profile CRUD, ordering, active selection, grouping, dedupe, and ProfileEx state.
- Server table and profile dialogs wired through typed IPC wrappers.
- Share-link parsers and exporters for supported profile protocols.
- Manual import and subscription flows, including base64, multi-URL, conversion target, filtering, and dedupe.
- Generated IPC binding drift.

## Command Results

Final local results:

```sh
cargo test --workspace --all-targets
```

PASS. Workspace tests completed successfully across `voya-app`, `voya-core`, `voya-db`, `voya-net`, `voya-platform`, `voya-udptest`, and `src-tauri`.

```sh
pnpm typecheck
```

PASS. TypeScript project references compiled with `tsc -b --pretty false`.

```sh
pnpm test -- --run
```

PASS. Vitest completed 2 test files and 7 tests.

```sh
pnpm lint
```

PASS after fixing the subscription dialogs to avoid unstable hook dependencies and synchronous form state updates in an effect.

```sh
pnpm bindings:check
```

PASS. `scripts/bindings.mjs --check` regenerated bindings in a temporary path and reported that `src/ipc/bindings.ts` is up to date.

```sh
test -f docs/verification/m1-profile-gate.md
```

PASS. This report is present.

## Fixes Applied

- Stabilized the subscription import dialog's subscription list memo so the target-label lookup does not receive a new empty array every render.
- Updated the subscription management dialog to load selected rows directly from user actions and save results instead of synchronizing editable form state with a synchronous `useEffect` setter.

## Deferred Edge Cases

These items do not block config generation:

- Real third-party subscription URLs were not used. Subscription networking is covered by deterministic local fixture HTTP servers to avoid leaking private URLs or depending on external services.
- QR scanning and real clipboard/file picker integration remain runtime/UI smoke concerns. Text import, file-content import, and typed IPC wrappers are covered locally.
- Parser correctness is currently asserted by round-trip, negative, and v2rayN parity tests for the Phase 02 surface. Generated-core JSON parity begins in Phase 03, where profile entities feed Xray and sing-box config generation.
- No real Tauri desktop session was launched in this gate. Persistence and IPC command behavior are covered by Rust tests and generated binding checks; end-to-end desktop smoke checks remain scheduled for later runtime phases.
