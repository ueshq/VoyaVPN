# Batch 08-01-tauri-packaging: Tauri Packaging Config

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
- Kind: `code`
- Execution: `codex`

## Batch Goal
Configure Tauri bundle targets, updater settings, sidecar strategy, attribution, and first-run core download posture.

## Depends On
- None

## Deliverables
- Tauri bundle configuration for macOS, Windows, and Linux targets.
- Updater configuration with placeholders for keys and channels.
- Attribution and licenses screen or document.
- First-run core download flow documentation.

## Acceptance
- Debug package build can run locally without signing credentials.
- GPL or AGPL cores are not bundled by default.

## Evidence To Capture
- docs/release/packaging.md.

## Verification Commands (must pass before declaring success)
- `pnpm tauri:build --debug`
- `test -f docs/release/packaging.md`

## Likely Files
- `src-tauri/**`
- `src/features/about/**`
- `docs/release/packaging.md`
- `package.json`

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
- If platform prerequisites are missing, document the exact failure and keep config changes deterministic.

## Working Agreement
- Finish only this batch and the minimum supporting work required for its verification commands.
- Read the source-of-truth files before changing behavior that must match v2rayN.
- Capture any skipped external checks in docs with a concrete reason and follow-up.

## Retry Context
The previous attempt for batch `08-01-tauri-packaging` failed verification.
Retry number: 1

Fix the implementation so that every verification command passes.
Before you finish, rerun the verification commands yourself and confirm they are green.

### Failed Check 1
Command: `pnpm tauri:build --debug`
Exit code: `2`
Output:
```text
[ELIFECYCLE] Command failed with exit code 2.
$ tauri build --debug
error: invalid value '1' for '--ci'
  [possible values: true, false]

For more information, try '--help'.
```
