# Batch 01-02-app-shell-design-system: App Shell And Design System

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
Build the navigable empty app shell with menubar, tabs, status bar, modal host, toaster, i18n, RTL-ready theme, and static tray.

## Depends On
- `01-01-workspace-scaffold`

## Deliverables
- React AppShell components and stores.
- shadcn/ui base components used by the shell.
- i18next setup with initial locale files.
- Theme and accent persistence stubs.
- Static Rust tray menu.

## Acceptance
- The first screen is the usable app shell, not a landing page.
- Tabs for Profiles, Clash Proxies, Clash Connections, and Logs exist even if empty.
- RTL locale plumbing is present.

## Evidence To Capture
- Frontend smoke test or component test for AppShell.

## Verification Commands (must pass before declaring success)
- `pnpm typecheck`
- `pnpm test -- --run`

## Likely Files
- `src/**`
- `src-tauri/**`

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
- Keep UI quiet and operational; avoid marketing-style hero content.

## Working Agreement
- Finish only this batch and the minimum supporting work required for its verification commands.
- Read the source-of-truth files before changing behavior that must match v2rayN.
- Capture any skipped external checks in docs with a concrete reason and follow-up.

## Retry Context
The previous attempt for batch `01-02-app-shell-design-system` failed verification.
Retry number: 1

Fix the implementation so that every verification command passes.
Before you finish, rerun the verification commands yourself and confirm they are green.

### Failed Check 1
Command: `pnpm test -- --run`
Exit code: `130`
Output:
```text
[1m[30m[44m DEV [49m[39m[22m [34mv4.1.7 [39m[90m/Users/afu/Dev/refs/VoyaVPN[39m

 [32m✓[39m src/App.test.tsx [2m([22m[2m2 tests[22m[2m)[22m[33m 505[2mms[22m[39m
     [33m[2m✓[22m[39m renders the app shell tabs and status bar [33m 351[2mms[22m[39m

[2m Test Files [22m [1m[32m1 passed[39m[22m[90m (1)[39m
[2m      Tests [22m [1m[32m2 passed[39m[22m[90m (2)[39m
[2m   Start at [22m 10:50:30
[2m   Duration [22m 3.01s[2m (transform 229ms, setup 177ms, import 815ms, tests 505ms, environment 1.22s)[22m

[1m[30m[42m PASS [49m[39m[22m [32mWaiting for file changes...[39m
       [2mpress [22m[1mh[22m[2m to show help[22m[2m, [22m[2mpress [22m[1mq[22m[2m to quit[22m
[31mCancelling test run. Press CTRL+c again to exit forcefully.
[39m
[ELIFECYCLE] Test failed. See above for more details.
$ vitest -- --run
[vite:react-swc] We recommend switching to `@vitejs/plugin-react` for improved performance as no swc plugins are used. More information at https://vite.dev/rolldown
```
