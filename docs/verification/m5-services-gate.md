# M5 Services Gate Verification

Batch: `06-05-services-phase-gate`

## Scope

- Clash API and UI workflows: proxy listing, delay tests, active selection, forced reload, rule-mode PATCH, traffic events, and connection events.
- Speedtest workflows: Tcping, Realping, UdpTest, Speedtest, Mixedtest, FastRealping, cancellation, and `ProfileExItem` result writing.
- Download and update workflows: app/core/geo/SRS target planning, GitHub release parsing, asset templating, proxy-to-direct fallback, and safe staged binary swap.
- Ruleset and geo workflows: `.dat` and `.srs` acquisition planning, source settings, local asset discovery, and sing-box config integration.

## Automated Gate

The service integration checks are deterministic and do not require live internet, public test endpoints, GitHub, or running Clash-compatible cores.

- Rust workspace tests use injected managers, fixture releases, local HTTP fixtures, packet fixtures, and local SOCKS5 UDP relay tests.
- Frontend tests mock typed IPC wrappers and verify UI workflows through generated binding shapes.
- Binding drift is checked by generating IPC bindings into a temporary file and comparing them with `src/ipc/bindings.ts`.

Required commands for this gate:

```sh
cargo test --workspace --all-targets
pnpm typecheck
pnpm test -- --run
pnpm lint
pnpm bindings:check
test -f docs/verification/m5-services-gate.md
```

Local result for this batch run: all required commands passed.

## Manual Live-Network Smoke

These checks are intentionally manual because they require real core binaries, a reachable profile, and real network conditions.

1. Start `pnpm tauri dev`, connect a known working profile, and verify the status bar reports running core state, logs, and traffic.
2. Open Clash Proxies against a running mihomo or sing-box Clash API, run a delay test, switch an active proxy, change rule mode, and force reload.
3. Open Clash Connections, confirm live traffic and connection rows update, then close one connection and verify it disappears or reports closed.
4. Run Fast, TCP, Real, UDP, Speed, and Mixed speed tests for a selected profile, then cancel a running test and verify row delay/speed/message/IP info updates.
5. Run update checks with a reachable GitHub connection and with a deliberately failing proxy to confirm direct fallback still works.
6. Acquire geo `.dat` and sing-box `.srs` assets, reconnect with routing/DNS rules that reference them, and verify generated configs use the resolved local paths.

## Related Evidence

- `docs/verification/clash.md`
- `docs/verification/speedtest.md`
- `docs/verification/updates.md`
- `docs/verification/ruleset-geo.md`
