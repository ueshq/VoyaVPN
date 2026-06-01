# Batch 05-05-regional-presets: Regional Presets

You are implementing the rollout `voyavpn-full-rewrite` in the repository rooted at `/Users/afu/Dev/refs/VoyaVPN`.

## Phase
- `05-routing-dns-tun-groups` — Routing, DNS, TUN, Groups
- Goal: Complete routing settings, DNS settings, TUN polish, policy group UI, proxy chain UI, and regional presets.
- Context: This phase deepens runtime control and exposes advanced generator features through the UI.

## Phase Entry Criteria
- Runtime alpha can connect and show state through real IPC.

## Phase Exit Criteria
- Routing, DNS, TUN, policy groups, proxy chains, and presets work in both generators and UI.

## Phase Risks
- Advanced generator UI can diverge from backend structures without typed forms and tests.

## Batch Shape
- Kind: `code`
- Execution: `codex`

## Batch Goal
Implement Russia and Iran regional preset application with external DNS template fetch, routing and DNS writes, and fallback behavior.

## Depends On
- `05-02-dns-settings`

## Deliverables
- Preset manager using voya-net.
- Preset UI actions and confirmation flow.
- Tests for successful fetch, null fallback, routing write, DNS write, and simple DNS behavior.

## Acceptance
- Preset apply fetches DNS templates through configured sources when available.
- Fallback enables custom DNS when network template data is unavailable.

## Evidence To Capture
- docs/verification/regional-presets.md.

## Verification Commands (must pass before declaring success)
- `cargo test -p voya-app preset --all-targets`
- `cargo test -p voya-net --all-targets`
- `pnpm typecheck`
- `test -f docs/verification/regional-presets.md`

## Likely Files
- `crates/voya-app/**`
- `crates/voya-net/**`
- `src/features/options/**`
- `docs/verification/regional-presets.md`

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
- Regional presets depend on voya-net and are not a static local-only settings write.

## Working Agreement
- Finish only this batch and the minimum supporting work required for its verification commands.
- Read the source-of-truth files before changing behavior that must match v2rayN.
- Capture any skipped external checks in docs with a concrete reason and follow-up.
