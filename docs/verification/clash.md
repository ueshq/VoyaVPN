# Clash API And UI Verification

Batch: `06-01-clash-api-ui`

## Scope

- Added generated IPC coverage for Clash REST commands and transient WebSocket events.
- Wired Clash Proxies and Clash Connections tabs to typed IPC wrappers.
- Routed `/traffic` and `/connections` WebSocket payloads into the runtime event store.
- Kept v2rayN Clash API parity for rule-mode `PATCH /configs` and forced reload `PUT /configs?force=true`.

## Automated Evidence

- `cargo test -p voya-net clash --all-targets` passed.
  - Covers REST request shaping for rule-mode PATCH, reload force query, delay URL encoding, and WebSocket message decoding.
- `cargo test -p voya-app clash --all-targets` passed.
  - Covers manager command behavior, active proxy selection, delay results, forced reload, and WebSocket event routing.
- `pnpm typecheck` passed.
- `pnpm test -- --run` passed.
  - Includes runtime store coverage for Clash traffic and connection transient events.
- `test -f docs/verification/clash.md` passes once this evidence file is present.
- Additional drift/lint checks:
  - `pnpm bindings:check` passed.
  - `pnpm lint` passed.

## External Checks

- No live mihomo/sing-box Clash API process was required for this batch. Automated checks use mocked HTTP transports and decoded WebSocket fixtures to avoid flaky network/runtime dependencies.
