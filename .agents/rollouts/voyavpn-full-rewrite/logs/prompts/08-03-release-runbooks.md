# Batch 08-03-release-runbooks: Release Runbooks

You are implementing the rollout `voyavpn-full-rewrite` in the repository rooted at `/Users/afu/Dev/refs/VoyaVPN`.

## Phase
- `08-packaging-release` — Packaging And Release
- Goal: Prepare package builds, updater metadata, release CI, runbooks, and final evidence for public beta.
- Context: This phase makes the app shippable while keeping credentials and real publication outside the runner.

## Phase Entry Criteria
- Feature-complete app and smoke checks are available.

## Phase Exit Criteria
- Debug or unsigned packages build, release workflows are configured, and manual signing or publication steps are documented.

## Phase Risks
- Packaging can depend on credentials or OS environments unavailable to the runner.

## Batch Shape
- Kind: `docs`
- Execution: `codex`

## Batch Goal
Write manual runbooks for signing, notarization, updater keys, OS smoke testing, rollback, and beta publication.

## Depends On
- `08-02-release-ci`

## Deliverables
- docs/release/runbook.md.
- docs/release/signing-notarization.md.
- docs/release/os-smoke-matrix.md.
- docs/release/rollback.md.

## Acceptance
- Every manual release checkpoint has owner, system, verification, and rollback notes.
- The runbook separates local debug packaging from public beta publication.

## Evidence To Capture
- Release docs exist and link to verification evidence.

## Verification Commands (must pass before declaring success)
- `test -f docs/release/runbook.md`
- `test -f docs/release/signing-notarization.md`
- `test -f docs/release/os-smoke-matrix.md`
- `test -f docs/release/rollback.md`

## Likely Files
- `docs/release/**`

## Sources Of Truth
- `.agents/rollouts/voyavpn-full-rewrite/spec.md`
- `.agents/rollouts/voyavpn-full-rewrite/plan.md`
- `/Users/afu/.claude/plans/typescript-shadcn-tauri-silly-marble.md`
- `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib`
- `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib.Tests`
- `/Users/afu/Dev/refs/v2rayN/v2rayN/v2rayN`
- `/Users/afu/Dev/refs/v2rayN/v2rayN/v2rayN.Desktop`

## Planning Notes
- This is a greenfield rewrite in VoyaVPN; the v2rayN repo is read-only reference material.
- Deliver subsystem by subsystem with backend, frontend, tests, and IPC wiring in the same slice when feasible.
- Keep all three platforms in scope from the first scaffold.
- Fresh SQLite schema only; no migration from v2rayN data and no obsolete columns.

## Success Metrics
- Rust workspace tests pass with cargo test --workspace --all-targets.
- Frontend checks pass with pnpm typecheck, pnpm test -- --run, and pnpm lint.
- Generated bindings have no drift after regeneration.
- Xray and sing-box generated configs match v2rayN golden fixtures and pass core acceptance where binaries exist.
- A real server can connect through pnpm tauri dev with logs, stats, and traffic flow.

## Global Context
- Target stack: Tauri 2, Rust, React, TypeScript, Tailwind v4, shadcn/ui, Zustand, TanStack Query, TanStack Table, Radix, i18next, sqlx, specta, tauri-specta.
- Rust crate layout: voya-core, voya-db, voya-platform, voya-net, voya-udptest, voya-app, and src-tauri.
- Frontend IPC rule: only src/ipc may import @tauri-apps/api; all app code uses typed wrappers.
- Config generator correctness is judged by generated core JSON and core acceptance, not entity snapshots alone.

## Hard Rules
- Do not modify /Users/afu/Dev/refs/v2rayN/v2rayN or sibling reference sources.
- Do not add obsolete v2rayN columns or data migration code.
- Do not place OS-specific code in voya-core.
- Do not hand-write TypeScript IPC DTOs that should be generated from Rust.
- Do not import @tauri-apps/api outside src/ipc.
- Do not redistribute GPL or AGPL core binaries in installers by default.
- Keep diffs focused on the current batch and update tests or docs for the touched surface.

## Batch Context
- Do not invent secret values; document the names and how owners supply them.

## Working Agreement
- Finish only this batch and the minimum supporting work required for its verification commands.
- Read the source-of-truth files before changing behavior that must match v2rayN.
- Capture any skipped external checks in docs with a concrete reason and follow-up.
