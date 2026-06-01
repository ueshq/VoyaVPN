# Statistics Verification

Batch: `04-05-statistics-speed`

## Implemented

- Xray statistics polling reads `http://127.0.0.1:{StatePort}/debug/vars`.
- sing-box statistics connects to `ws://127.0.0.1:{StatePort2}/traffic` after the v2rayN-style initial delay.
- Both services are started together and gate their hot path on the active running core family:
  - Xray family: `Xray`, `v2fly`, `v2fly_v5`.
  - sing-box family: `sing_box`, `mihomo`.
- Statistics events are coalesced around one second.
- Display speed includes proxy plus direct traffic.
- Persistent traffic is written to the active profile only and uses proxy traffic for per-server totals.
- `server_stat_items` supports date rollover, orphan cleanup, and clone-on-profile-copy behavior.
- Profile list rows now include generated `serverStat` data, and the server table renders today/total upload/download columns.

## Local Evidence

- `cargo test -p voya-app statistics --all-targets`
- `cargo test -p voya-db statistics --all-targets`
- `pnpm typecheck`
- `pnpm test -- --run`
- `test -f docs/verification/statistics.md`

## Notes

- Binding generation initially failed because the local filesystem had only about 69 MiB free. `cargo clean` removed local build artifacts, freeing about 7.6 GiB, and `pnpm bindings` then regenerated `src/ipc/bindings.ts`.
- No external core binaries were launched in this batch; parser, keying, rollover, and UI wiring are covered by local deterministic tests.
