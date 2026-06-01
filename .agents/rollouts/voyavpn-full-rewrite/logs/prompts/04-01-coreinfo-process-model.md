# Batch 04-01-coreinfo-process-model: Core Info And Process Model

You are implementing the rollout `voyavpn-full-rewrite` in the repository rooted at `/Users/afu/Dev/refs/VoyaVPN`.

## Phase
- `04-runtime-alpha` — Runtime Alpha
- Goal: Deliver connect, disconnect, core process supervision, logs, system proxy, tray, and statistics.
- Context: This phase turns generated configs into a usable internal alpha.

## Phase Entry Criteria
- Xray and sing-box configs can be generated from persisted profiles.

## Phase Exit Criteria
- A real server can connect, traffic flows, logs stream, proxy mode toggles, and speed is visible.

## Phase Risks
- Privilege, process tree, and route cleanup bugs can leave the host in a bad state.

## Batch Shape
- Kind: `code`
- Execution: `codex`

## Batch Goal
Port the 15-core launch table, executable discovery, per-core arguments, env vars, and app/bin/log path resolution.

## Depends On
- None

## Deliverables
- voya-platform app dir, bin dir, log dir, temp dir, and portable-mode detection.
- voya-app or voya-platform CoreInfo table for all supported cores.
- Executable discovery and chmod behavior on Unix.
- Tests for command templates and OS path behavior.

## Acceptance
- CoreInfo covers xray, v2fly variants, mihomo, hysteria, naiveproxy, tuic, sing-box, juicity, brook, overtls, shadowquic, mieru, and v2rayN core entries.
- Argument substitution and env vars match the reference plan.

## Evidence To Capture
- docs/verification/coreinfo.md.

## Verification Commands (must pass before declaring success)
- `cargo test -p voya-platform coreinfo --all-targets`
- `cargo test -p voya-app coreinfo --all-targets`
- `test -f docs/verification/coreinfo.md`

## Likely Files
- `crates/voya-platform/**`
- `crates/voya-app/**`
- `docs/verification/coreinfo.md`

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
- Pay attention to mihomo executable probe order and env vars for v2fly, xray, and mieru.

## Working Agreement
- Finish only this batch and the minimum supporting work required for its verification commands.
- Read the source-of-truth files before changing behavior that must match v2rayN.
- Capture any skipped external checks in docs with a concrete reason and follow-up.
