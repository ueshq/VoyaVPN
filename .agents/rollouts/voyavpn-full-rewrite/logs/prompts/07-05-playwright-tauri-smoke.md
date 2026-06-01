# Batch 07-05-playwright-tauri-smoke: Playwright And Tauri Smoke

You are implementing the rollout `voyavpn-full-rewrite` in the repository rooted at `/Users/afu/Dev/refs/VoyaVPN`.

## Phase
- `07-polish-backup-i18n` — Backup, Integrations, I18n, Polish
- Goal: Complete backup, WebDAV, autostart, hotkeys, QR, i18n, theming, accessibility, performance, and smoke automation.
- Context: This phase closes user-facing breadth and quality gates before packaging.

## Phase Entry Criteria
- Major runtime and service workflows are implemented.

## Phase Exit Criteria
- The UI and integration surface is complete, localized, accessible, and smoke-tested where automatable.

## Phase Risks
- Polish work can sprawl; each batch must stay tied to specific workflows and checks.

## Batch Shape
- Kind: `code`
- Execution: `codex`

## Batch Goal
Add automated smoke coverage for key app flows through Playwright and tauri-driver where locally feasible.

## Depends On
- `07-04-theme-a11y-perf`

## Deliverables
- Playwright setup for frontend flows.
- Tauri driver smoke setup or documented platform limitations.
- Smoke tests for app shell, profile add, import fixture, connect fake, routing, DNS, and dialogs.
- Manual smoke matrix for OS-only flows.

## Acceptance
- Frontend smoke tests run non-interactively.
- Tauri-driver gaps are documented with exact manual checks.

## Evidence To Capture
- docs/verification/cross-platform-smoke.md.

## Verification Commands (must pass before declaring success)
- `pnpm test -- --run`
- `pnpm typecheck`
- `test -f docs/verification/cross-platform-smoke.md`

## Likely Files
- `tests/**`
- `e2e/**`
- `playwright.config.*`
- `docs/verification/cross-platform-smoke.md`

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
- Do not require real proxies or real OS proxy changes in automated smoke tests.

## Working Agreement
- Finish only this batch and the minimum supporting work required for its verification commands.
- Read the source-of-truth files before changing behavior that must match v2rayN.
- Capture any skipped external checks in docs with a concrete reason and follow-up.
