# Golden Export Report

Batch: `03-06-golden-export-harness`

## Scope

The golden harness now has a manifest-driven fixture matrix at `tests/golden/matrix.json`.
Each case points at a reference JSON fixture, a Rust generated-output selector, v2rayN source paths, hotspot tags, and any future volatile-field rules.

Current coverage:

- Xray: xhttp/TLS/finalmask outbound, policy-group least-load observatory, inbounds/stats/TUN, advanced DNS/routing, and FullConfigTemplate TUN proxy-detour merge.
- sing-box: VLESS websocket/TLS/mux outbound, ProxyChain detour, selector/urltest policy group, typed DNS fakeip schema, DNS-derived rulesets, TUN inbound, and TUN route template.

Golden comparisons recursively sort object keys, preserve array order, preserve null-vs-missing semantics, and print a case-scoped unified diff when generated JSON drifts.

## Export Path

The v2rayN reference checkout remains read-only. Reference export should be done by copying the example harness in `scripts/golden/VoyaGoldenExportHarness.cs.example` into a temporary copy of:

- `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib.Tests`

The intended source helpers are:

- `ServiceLib.Tests/CoreConfig/CoreConfigTestFactory.cs`
- `ServiceLib/Handler/CoreConfigHandler.cs`
- `ServiceLib/Services/CoreConfig/V2ray/**`
- `ServiceLib/Services/CoreConfig/Singbox/**`

Do not write generated fixtures directly into the v2rayN repo. Export into a temporary directory, review the canonical JSON, then copy selected fixture updates into `tests/golden`.

## Core Acceptance

Local unit tests do not require external core binaries. Optional acceptance is opt-in:

```sh
VOYA_GOLDEN_ACCEPTANCE=1 cargo test -p voya-core golden_core_acceptance --all-targets -- --nocapture
```

Binary discovery:

- Xray: `VOYA_XRAY_BIN=/path/to/xray`, otherwise `xray` from `PATH`
- sing-box: `VOYA_SINGBOX_BIN=/path/to/sing-box`, otherwise `sing-box` from `PATH`

If a binary is missing, the acceptance test prints a skip reason and passes. The concrete reason is that GPL/MPL/AGPL core binaries are external runtime dependencies and are not redistributed in this repository. Follow-up for CI is to install approved binaries in a dedicated opt-in acceptance job and set the two environment variables.

## Verification

Required batch commands:

```sh
cargo test -p voya-core golden --all-targets
test -f docs/verification/golden-report.md
```
