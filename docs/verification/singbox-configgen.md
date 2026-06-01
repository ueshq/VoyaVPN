# Sing-Box Config Generation Verification

Batch: `03-05-singbox-coregen-routing-dns`

## Scope

- Added typed sing-box DNS models for `servers`, `rules`, `domain_resolver`, `predefined`, fakeip ranges, and `independent_cache`.
- Added deterministic DNS generation for bootstrap, direct, remote, hosts, fakeip, predefined host answers, binding-query blocks, final-DNS direct detection, and routing-derived DNS rules.
- Added sing-box routing generation for default domain resolver, TUN rules, DNS hijack/sniff rules, ICMP handling, host resolve rules, final outbound, and per-rule outbound resolution.
- Added sing-box `rule_set` conversion for route and DNS `geosite`/`geoip` matches with remote SRS references.
- Kept template behavior aligned with the existing sing-box template service, including TUN template selection and proxy-detour application.

## Golden Fixtures

- `tests/golden/singbox/dns/fakeip_typed.json`
- `tests/golden/singbox/inbounds/tun.json`
- `tests/golden/singbox/route/rulesets_from_dns.json`
- `tests/golden/singbox/route/tun.json`
- Existing outbound fixtures remain under `tests/golden/singbox/outbounds/`.

## Verification

Passed:

```sh
cargo test -p voya-core singbox --all-targets
test -f docs/verification/singbox-configgen.md
```

External sing-box core acceptance was not run in this batch because the repository does not redistribute or vendor core binaries. Follow-up runtime batches should run `sing-box check -c` against emitted fixture configs when a local `sing-box` binary is available.
