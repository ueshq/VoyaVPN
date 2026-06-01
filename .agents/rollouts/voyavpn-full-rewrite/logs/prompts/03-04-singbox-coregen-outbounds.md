# Batch 03-04-singbox-coregen-outbounds: Sing-Box Outbounds And Selectors

You are implementing the rollout `voyavpn-full-rewrite` in the repository rooted at `/Users/afu/Dev/refs/VoyaVPN`.

## Phase
- `03-config-generation` — Xray And Sing-Box Config Generation
- Goal: Port deterministic config generation for Xray and sing-box with golden parity and core acceptance gates.
- Context: This is the highest-fidelity phase and should move one resolver or generator area at a time.

## Phase Entry Criteria
- Profiles, parser models, DB, and typed IPC are stable.

## Phase Exit Criteria
- Xray and sing-box golden parity is established for the fixture matrix.

## Phase Risks
- Small serialization differences can produce valid but behaviorally wrong configs.
- Policy group, proxy chain, DNS, finalmask, and TUN behavior are especially easy to drift.

## Batch Shape
- Kind: `code`
- Execution: `codex`

## Batch Goal
Implement sing-box outbound generation for protocols, transports, selectors, urltest, mux, detour, and clash API basics.

## Depends On
- `03-01-context-builder`

## Deliverables
- sing-box serde config models with snake_case and strict skip rules.
- Outbound and transport generation for live protocol matrix.
- Selector and urltest policy group generation.
- ProxyChain detour behavior.
- Experimental clash_api and cache_file scaffolding.

## Acceptance
- Generated JSON avoids null or unknown fields rejected by sing-box.
- Selector ordering and dedupe match policy group expectations.

## Evidence To Capture
- docs/verification/singbox-outbounds.md.

## Verification Commands (must pass before declaring success)
- `cargo test -p voya-core singbox_outbound --all-targets`
- `cargo test -p voya-core singbox_selector --all-targets`
- `test -f docs/verification/singbox-outbounds.md`

## Likely Files
- `crates/voya-core/**`
- `tests/golden/**`
- `docs/verification/singbox-outbounds.md`

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
- Keep sing-box strict serialization in mind; unknown or null fields can fail check -c.

## Working Agreement
- Finish only this batch and the minimum supporting work required for its verification commands.
- Read the source-of-truth files before changing behavior that must match v2rayN.
- Capture any skipped external checks in docs with a concrete reason and follow-up.
