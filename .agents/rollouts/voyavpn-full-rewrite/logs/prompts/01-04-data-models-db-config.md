# Batch 01-04-data-models-db-config: Data Models, DB, Config

You are implementing the rollout `voyavpn-full-rewrite` in the repository rooted at `/Users/afu/Dev/refs/VoyaVPN`.

## Phase
- `01-foundation` — Workspace, Shell, IPC, DB
- Goal: Create the Rust, Tauri, React, typed IPC, data, event, and CI foundation.
- Context: This phase establishes the repo shape that all subsystem work builds on.

## Phase Entry Criteria
- Baseline docs and ADRs exist.

## Phase Exit Criteria
- Workspace compiles, frontend checks run, generated IPC exists, DB migrations exist, and CI covers baseline checks.

## Phase Risks
- Bad early boundaries can force expensive refactors during config-gen or platform work.

## Batch Shape
- Kind: `code`
- Execution: `codex`

## Batch Goal
Port live model shapes, fresh SQLite schema, repositories, JSON config defaults, and typed blob boundary.

## Depends On
- `01-03-typed-ipc-events`

## Deliverables
- voya-core models and enums with serde and specta derives.
- voya-db migrations, repositories, and typed JSON blob helpers.
- AppConfig defaults and load/save commands.
- Unit and integration tests for DB defaults and persistence.

## Acceptance
- Obsolete columns are absent.
- Enum discriminants match the planning source.
- Settings persist across process restart in tests.

## Evidence To Capture
- docs/verification/db-schema.md with schema notes.

## Verification Commands (must pass before declaring success)
- `cargo test -p voya-core --all-targets`
- `cargo test -p voya-db --all-targets`
- `pnpm bindings:check`

## Likely Files
- `crates/voya-core/**`
- `crates/voya-db/**`
- `src-tauri/**`
- `src/ipc/**`
- `docs/verification/db-schema.md`

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
- ProtocolExtraItem and TransportExtraItem stay typed across IPC and become TEXT only inside voya-db blob helpers.

## Working Agreement
- Finish only this batch and the minimum supporting work required for its verification commands.
- Read the source-of-truth files before changing behavior that must match v2rayN.
- Capture any skipped external checks in docs with a concrete reason and follow-up.
