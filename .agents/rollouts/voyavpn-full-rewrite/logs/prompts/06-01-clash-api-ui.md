# Batch 06-01-clash-api-ui: Clash API And UI

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
- Kind: `code`
- Execution: `codex`

## Batch Goal
Implement Clash REST and WebSocket clients plus proxies and connections screens.

## Depends On
- None

## Deliverables
- voya-net Clash REST and WebSocket client.
- voya-app Clash manager and commands.
- Clash Proxies and Clash Connections tabs.
- Delay test, select active, connection monitor, close connection, rule-mode PATCH, and reload force behavior.

## Acceptance
- Rule-mode switch uses HTTP PATCH on /configs.
- Reload uses /configs?force=true.
- WebSocket traffic and connection events update UI stores.

## Evidence To Capture
- docs/verification/clash.md.

## Verification Commands (must pass before declaring success)
- `cargo test -p voya-net clash --all-targets`
- `cargo test -p voya-app clash --all-targets`
- `pnpm typecheck`
- `pnpm test -- --run`
- `test -f docs/verification/clash.md`

## Likely Files
- `crates/voya-net/**`
- `crates/voya-app/**`
- `src/features/clash/**`
- `docs/verification/clash.md`

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
- Use mocked HTTP and WebSocket clients in automated tests.

## Working Agreement
- Finish only this batch and the minimum supporting work required for its verification commands.
- Read the source-of-truth files before changing behavior that must match v2rayN.
- Capture any skipped external checks in docs with a concrete reason and follow-up.
