# M2 Config Generation Gate

Batch: `03-07-configgen-phase-gate`

## Scope

This gate stabilizes the M2 config-generation surface after the context builder, Xray generator, sing-box generator, templates, golden harness, and generated IPC batches.

Covered automated surfaces:

- `voya-core` context builder resolution, validation, protect-domain collection, proxy-chain expansion, pre-socks handling, and rule outbound lookup.
- Xray generation for outbound transports, finalmask, policy groups, inbounds, stats, DNS, routing, TUN, and full-config templates.
- sing-box generation for outbound transports, proxy-chain detour, selector/urltest groups, typed DNS, fakeip, rulesets, inbounds, TUN, and route templates.
- Golden fixture loading, canonical comparison, actionable diff rendering, and opt-in core acceptance skip behavior.
- Workspace compatibility after config-generation model and IPC additions.

## Golden Coverage

The fixture manifest at `tests/golden/matrix.json` contains 12 config-generation cases:

- Xray: 5 cases covering VLESS xhttp/TLS/finalmask, least-load policy group observatory, inbounds/stats/TUN, advanced DNS/routing, and FullConfigTemplate TUN proxy-detour merge.
- sing-box: 7 cases covering VLESS websocket/TLS/mux, ProxyChain detour, selector/urltest policy group, typed fakeip DNS, DNS-derived rulesets, TUN inbound, and TUN route behavior.

The detailed golden export evidence remains in `docs/verification/golden-report.md`. That report lists fixture purpose, source reference paths, export workflow, and optional core acceptance handling.

## Known Gaps

- External Xray and sing-box binary acceptance is not part of the default gate. Core binaries are not vendored or redistributed in this repository, and the golden manifest keeps `core_acceptance` disabled for the current fixtures. Follow-up: add an opt-in CI job with approved local binaries and set `VOYA_GOLDEN_ACCEPTANCE=1`.
- The golden corpus targets the highest-risk M2 behaviors but is not an exhaustive protocol matrix. Follow-up config-generation batches should add cases for additional protocol and routing combinations when those surfaces are expanded.
- The v2rayN export helper is documented as an example harness under `scripts/golden/`; it must be copied into a temporary checkout when refreshing fixtures. The read-only reference checkout remains untouched.

## Gate Evidence

Verified on 2026-06-01 in the local VoyaVPN workspace:

```sh
cargo test -p voya-core --all-targets
cargo test --workspace --all-targets
pnpm bindings:check
test -f docs/verification/m2-configgen-gate.md
```

All required gate commands passed. The Rust workspace emitted no config-generation warnings after moving the sing-box test-only `RoutingItem` import into the test module.
