# ADR 0003: Config Generation Parity

Status: Accepted

Date: 2026-05-31

## Context

Config generation is the highest-risk parity area. v2rayN builds config context in `ServiceLib/Handler/Builder/CoreConfigContextBuilder.cs`, then emits Xray and sing-box JSON through `ServiceLib/Services/CoreConfig/V2ray/**` and `ServiceLib/Services/CoreConfig/Singbox/**`. Existing reference tests under `ServiceLib.Tests/CoreConfig/**` assert important behavior such as policy groups, proxy chains, routing, and core-specific output.

Correctness is judged by generated core JSON and core acceptance, not by entity snapshots alone.

## Decision

`voya-core` owns deterministic config generation for Xray and sing-box:

- `coregen::context` ports `CoreConfigContextBuilder` behavior, including main/pre-socks contexts, subscription-level virtual proxy chains, group traversal, cycle/dedup handling, ECH SNI protection, xhttp download address protection, per-rule outbound resolution, and TUN/pre-socks context changes.
- `coregen::xray` ports Xray output, including inbounds, outbounds, routing, DNS, policy balancers/observatory, proxy-chain `dialerProxy`, templates, stats API/dokodemo inbound, Xray TUN inbound, bind/sendThrough, and finalmask behavior.
- `coregen::singbox` ports sing-box output, including selector/urltest groups, proxy-chain `detour`, rule sets, fakeip, typed DNS server schema, Clash API/cache file, mux, TUN, templates, bind/sendThrough, and route/DNS behavior.
- Core generation receives platform facts through explicit inputs rather than reading OS state directly.

Golden testing is the parity contract:

- Golden fixtures are exported from the read-only v2rayN reference behavior.
- Rust generation canonicalizes JSON and diffs generated output against the golden corpus.
- Golden fixtures must cover high-risk hotspots from `docs/fidelity-hotspots.md`: finalmask, policy group ordering, proxy chains, DNS final/direct detection, TUN, pre-socks, stats config, templates, and per-rule outbounds.
- Where binaries exist, generated Xray configs must pass `xray run -test` and generated sing-box configs must pass `sing-box check -c`.
- When core binaries are not available, tests must skip acceptance with explicit evidence and still run JSON golden parity.

Config generation must use live model fields only. There is no legacy migration, no obsolete database columns, and no generator dependency on obsolete v2rayN fields.

## Consequences

- Config generator tests must assert on final core JSON.
- Raw JSON is allowed only at defined template/raw config boundaries; normal profile, DNS, routing, transport, and protocol data should be typed.
- Platform-specific behavior that changes config output must be injected and fixture-tested for Windows, Linux, and macOS cases.
- Later implementation batches must not treat Xray and sing-box as separate feature quality levels; both are required parity targets.
