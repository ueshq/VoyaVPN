# Batch 06-05-services-phase-gate: Service Integrations Phase Gate

You are implementing the rollout `voyavpn-full-rewrite` in the repository rooted at `/Users/afu/Dev/refs/VoyaVPN`.

## Phase
- `06-service-integrations` — Clash, Speedtest, Updates
- Goal: Complete Clash API, speedtest, downloads, updates, ruleset, and geo acquisition workflows.
- Context: This phase adds operational services around a working proxy runtime.

## Phase Entry Criteria
- Runtime, routing, DNS, TUN, and groups are functional.

## Phase Exit Criteria
- Clash, speedtest, downloads, updates, rulesets, and geo acquisition are implemented and tested.

## Phase Risks
- Network-dependent behavior can make tests flaky unless clients are injectable and fixture-driven.

## Batch Shape
- Kind: `verification`
- Execution: `codex`

## Batch Goal
Stabilize Clash, speedtest, updates, ruleset, geo, and related UI workflows.

## Depends On
- `06-04-ruleset-geo`

## Deliverables
- docs/verification/m5-services-gate.md.
- Fixes required for service integration checks.

## Acceptance
- Automated service integration tests pass without live network dependency.
- Manual live-network smoke steps are documented.

## Evidence To Capture
- M5 services gate report.

## Verification Commands (must pass before declaring success)
- `cargo test --workspace --all-targets`
- `pnpm typecheck`
- `pnpm test -- --run`
- `pnpm lint`
- `pnpm bindings:check`
- `test -f docs/verification/m5-services-gate.md`

## Likely Files
- `docs/verification/m5-services-gate.md`
- `crates/**`
- `src/**`

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
- Do not hide real-network requirements in automated tests.

## Working Agreement
- Finish only this batch and the minimum supporting work required for its verification commands.
- Read the source-of-truth files before changing behavior that must match v2rayN.
- Capture any skipped external checks in docs with a concrete reason and follow-up.
