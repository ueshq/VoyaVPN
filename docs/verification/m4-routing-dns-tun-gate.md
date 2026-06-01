# M4 Advanced Routing Gate

Batch: `05-06-advanced-routing-phase-gate`

Date: 2026-06-01

## Scope

This gate stabilizes Phase 05: routing settings, DNS settings, TUN polish, policy group UI, proxy chain UI, and regional presets.

Covered subsystem evidence:

- `docs/verification/routing.md`
- `docs/verification/dns.md`
- `docs/verification/tun.md`
- `docs/verification/groups-chains.md`
- `docs/verification/regional-presets.md`

## Fixes Applied

- Fixed the policy group child picker so draft child selections reset when the picker is opened, without synchronously setting React state from an effect.
- Stabilized the child candidate memoization path used by group preview and selection rendering.

## Automated Gate Results

- `cargo test --workspace --all-targets` passed.
- `pnpm typecheck` passed.
- `pnpm test -- --run` passed.
- `pnpm lint` passed.
- `pnpm bindings:check` passed with generated IPC bindings up to date.
- `test -f docs/verification/m4-routing-dns-tun-gate.md` is the file-presence check for this report.

## Manual Or External Checks

Manual TUN OS smoke remains documented separately in `docs/verification/manual-os-smoke.md` and `docs/verification/tun.md`. It was not executed in this automated gate because it requires host route mutation, sudo or UAC credentials, installed third-party core binaries, and real Windows, Linux, and macOS network devices.

External Xray and sing-box binary acceptance checks are not part of this batch's required command list. Generator correctness for the advanced routing surface is covered here by the Rust golden/unit tests and frontend typed UI tests.
