# Batch 00-02-architecture-adrs: Architecture ADRs

You are implementing the rollout `voyavpn-full-rewrite` in the repository rooted at `/Users/afu/Dev/refs/VoyaVPN`.

## Phase
- `00-baseline` — Baseline And Evidence
- Goal: Establish source inventory, architecture decisions, and verification scaffolding before implementation starts.
- Context: This phase creates the human evidence needed to keep a full rewrite aligned with the reference app.

## Phase Entry Criteria
- The target VoyaVPN repo exists and may be empty.
- The v2rayN reference repo is available read-only.

## Phase Exit Criteria
- Reference source areas and high-risk parity points are documented.
- Architecture and verification decisions are captured in docs.

## Phase Risks
- Missing source inventory can cause later batches to silently drift from v2rayN behavior.

## Batch Shape
- Kind: `docs`
- Execution: `codex`

## Batch Goal
Record the core architecture decisions that all implementation batches must preserve.

## Depends On
- `00-01-baseline-inventory`

## Deliverables
- docs/adr/0001-target-architecture.md.
- docs/adr/0002-typed-ipc-and-events.md.
- docs/adr/0003-config-generation-parity.md.
- docs/adr/0004-platform-boundaries.md.

## Acceptance
- ADRs capture crate boundaries, generated bindings, event channels, DB stance, platform isolation, and golden testing.
- ADRs explicitly state no legacy migration and no obsolete columns.

## Evidence To Capture
- ADR index or docs references are present.

## Verification Commands (must pass before declaring success)
- `test -f docs/adr/0001-target-architecture.md`
- `test -f docs/adr/0002-typed-ipc-and-events.md`
- `test -f docs/adr/0003-config-generation-parity.md`
- `test -f docs/adr/0004-platform-boundaries.md`

## Likely Files
- `docs/adr/**`

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
- Keep ADRs short but decisive so later Codex batches can rely on them.

## Working Agreement
- Finish only this batch and the minimum supporting work required for its verification commands.
- Read the source-of-truth files before changing behavior that must match v2rayN.
- Capture any skipped external checks in docs with a concrete reason and follow-up.
