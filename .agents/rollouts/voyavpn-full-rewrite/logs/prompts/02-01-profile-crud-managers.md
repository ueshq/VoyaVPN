# Batch 02-01-profile-crud-managers: Profile CRUD Managers

You are implementing the rollout `voyavpn-full-rewrite` in the repository rooted at `/Users/afu/Dev/refs/VoyaVPN`.

## Phase
- `02-profiles-imports` — Profiles, Parsers, Subscriptions
- Goal: Deliver persisted profiles, server table, protocol dialogs, share links, imports, and subscription flows.
- Context: This phase turns the empty shell into a profile manager with real data and import paths.

## Phase Entry Criteria
- Typed IPC, data models, DB, and app shell are available.

## Phase Exit Criteria
- Users can create, edit, import, view, sort, dedupe, and persist profiles and subscriptions.

## Phase Risks
- Parser edge cases can corrupt later config generation if not tested early.

## Batch Shape
- Kind: `code`
- Execution: `codex`

## Batch Goal
Implement profile CRUD, ordering, active selection, dedupe, copy, move, grouping, and ProfileEx manager behavior.

## Depends On
- None

## Deliverables
- voya-app profile manager modules.
- DB repositories and IPC commands for profile operations.
- Invalidation events for profile changes.
- Rust tests for CRUD, reorder, defaulting, dedupe, and active selection.

## Acceptance
- All live profile types can be persisted through typed commands.
- Operations update state and emit expected invalidation events.

## Evidence To Capture
- docs/verification/profile-crud.md.

## Verification Commands (must pass before declaring success)
- `cargo test -p voya-app profile --all-targets`
- `cargo test -p voya-db profile --all-targets`
- `pnpm bindings:check`

## Likely Files
- `crates/voya-app/**`
- `crates/voya-db/**`
- `crates/voya-core/**`
- `src-tauri/**`
- `docs/verification/profile-crud.md`

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
- Use ConfigHandler behavior as the reference but decompose it into cohesive Rust modules.

## Working Agreement
- Finish only this batch and the minimum supporting work required for its verification commands.
- Read the source-of-truth files before changing behavior that must match v2rayN.
- Capture any skipped external checks in docs with a concrete reason and follow-up.
