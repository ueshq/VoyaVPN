# Clash Live Updates Resilience Verification

Batch: `04-01-verification-evidence`, `04-02-regression-sweep`

## Scope

- Clash monitor lifecycle is typed through generated IPC as `running`, `stopped`, and `failed` with `running`, `stale`, and optional `message` fields. `running` means monitor tasks are active, not proof that both WebSocket resources are currently connected.
- Runtime event store keeps the last Clash traffic and connection snapshots when the monitor stops or fails, marks cached live data stale, and clears stale state only after fresh live traffic or connection events.
- App shell lifecycle keeps the 100ms Clash-tab enter delay and 2s leave grace period, shares one monitor across Clash Proxies and Clash Connections, and records start/stop failures as failed stale state with a toast.
- Clash Proxies and Clash Connections render compact live/stale/failed monitor status without hiding traffic totals, search, refresh, close, reload, delay-test, or rule-mode controls.

## Automated Evidence

- `cargo test -p voya-app --all-targets clash` passed in rollout logs.
  - Covers Clash REST compatibility, WebSocket event routing, `ClashMonitorStatus` stale contract, no-runtime start failure, idempotent stop, duplicate start reuse, restart after stop, endpoint replacement, and cloned controller state.
- `pnpm bindings:check` passed after the monitor status/event contract changes.
  - Confirms generated TypeScript IPC bindings include the new Clash monitor lifecycle types and transient event payload.
- `pnpm typecheck` passed for the frontend changes in the backend, store, lifecycle, and UI batches.
- Vitest store coverage: `pnpm exec vitest --run src/ipc/runtime-event-store.test.ts` passed.
  - Covers monitor lifecycle store actions, stopped/failed snapshot preservation, failure messages, fresh traffic clearing stale state, and coalesced connection events clearing stale state.
- Vitest app/UI coverage: `pnpm exec vitest --run src/App.test.tsx` passed.
  - Covers deferred monitor start, delayed stop, rapid switching between Clash tabs, start/stop failure handling, stale monitor badges in both Clash screens, manual refresh and close mutations preserving stale state, selected-connection cleanup, and virtualization across stale/live transitions.

## Regression Sweep

- Final sweep run: `2026-06-06 02:37 CST`.
- Logs are stored under `.agents/rollouts/clash-live-updates-resilience/logs/04-02-regression-sweep/`.
- `cargo test -p voya-app --all-targets clash` passed.
  - Log: `cargo-test-voya-app-all-targets-clash.log`
  - Result: 13 passed, 0 failed, 66 filtered out.
- `pnpm bindings:check` passed.
  - Log: `pnpm-bindings-check.log`
  - Result: generated IPC bindings are up to date; no binding drift remains.
- `pnpm typecheck` passed after the lint fix.
  - Log: `pnpm-typecheck-rerun.log`
  - Result: `tsc -b --pretty false` exited 0.
- `pnpm exec vitest --run` passed after the lint fix.
  - Log: `pnpm-exec-vitest-run-rerun.log`
  - Result: 4 test files passed, 32 tests passed.
- `pnpm lint` passed after a scoped Clash badge fix.
  - Log: `pnpm-lint-rerun.log`
  - Result: 0 errors, 3 warnings. Warnings are existing/react-compiler compatibility warnings in `src/components/ui/badge.tsx`, `src/components/ui/button.tsx`, and TanStack Virtual usage in `src/features/clash/clash-connections-screen.tsx`.
- `bash -lc 'if rg "@tauri-apps/api" src | rg -v "^src/ipc/" -q; then exit 1; fi'` passed.
  - Log: `tauri-api-import-boundary.log`
  - Result: no `@tauri-apps/api` imports exist outside `src/ipc`; the log is empty by design.
- Initial `pnpm lint` found one Clash-rollout error in `src/features/clash/clash-monitor-status-badge.tsx`; the badge now renders imported Lucide icons directly instead of creating an icon component variable during render.
- No automated check requires a live mihomo or sing-box Clash API process.
- No verification command was skipped.

## Manual Live-Core Smoke

- Manual smoke remains outside the runner because it requires a real Clash-compatible core and generated traffic.
- Start `pnpm tauri:dev` against a running mihomo or sing-box Clash API process, open Clash Proxies, and confirm the monitor badge moves from starting to live while traffic data updates.
- Switch between Clash Proxies and Clash Connections and confirm the monitor stays live, connection rows update, selection remains valid, and large lists stay virtualized.
- Leave both Clash tabs for more than 2 seconds and confirm the last traffic and connection snapshots remain visible, the monitor badge reports stale/stopped, and controls remain usable.
- Use manual refresh while stale and confirm refreshed data can seed the view without claiming the monitor is live.
- Re-enter a Clash tab and confirm start success or fresh live events clear stale state. If start or stop fails, confirm the badge reports failed with the available message while cached snapshots remain visible.
