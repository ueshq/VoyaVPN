# Xray Config Generation Verification

Batch: `03-03-xray-coregen-routing-dns`

## Scope

- Added deterministic Xray generation for local mixed SOCKS inbounds, second local port, LAN auth port, dokodemo API inbound, stats/policy/metrics, and Xray TUN inbound.
- Added routing generation for TUN rules, user routing rules, per-rule outbound resolution, balancer rewrites, DNS outbound routing, and final-rule handling.
- Added simple and custom Xray DNS generation covering hosts, expected IPs, domain/strategy handling, protect domains, direct final-DNS auto-detect, direct-DNS routing tags, and fake DNS pool emission.
- Added full config template materialization for normal/TUN templates, proxy-only filtering, proxy-detour insertion, balancer/observatory merging, and template outbound append behavior.

## Golden Fixtures

- `tests/golden/xray/full/inbounds_stats_tun.json`
- `tests/golden/xray/full/advanced_dns_routing.json`
- `tests/golden/xray/full/template_tun_proxy_detour.json`
- Existing outbound fixtures remain under `tests/golden/xray/outbounds/`.

## Verification

Passed:

```sh
cargo test -p voya-core xray --all-targets
test -f docs/verification/xray-configgen.md
```

External Xray core acceptance was not run in this batch because the repository does not redistribute or vendor core binaries. Follow-up runtime batches should run `xray run -test` against emitted fixture configs when a local Xray binary is available.
