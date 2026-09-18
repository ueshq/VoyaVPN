# ADR 0003: Config Generation Parity

Status: Accepted

Date: 2026-05-31

Amended by [ADR 0008](0008-manual-node-groups.md): executable groups, user proxy chains and full configuration imports are retired.

## Context

Config generation is the highest-risk parity area. v2rayN builds config context in `ServiceLib/Handler/Builder/CoreConfigContextBuilder.cs`, then emits sing-box JSON through `ServiceLib/Services/CoreConfig/Singbox/**`. Existing reference tests under `ServiceLib.Tests/CoreConfig/**` assert important behavior such as policy groups, proxy chains, routing, and core-specific output.

Correctness is judged by generated sing-box JSON and core acceptance, not by entity snapshots alone.

## Decision

`voya-core` owns deterministic config generation for sing-box:

- `coregen::context` ports `CoreConfigContextBuilder` behavior, including main/pre-socks contexts, ECH SNI protection, per-rule outbound resolution, and TUN/pre-socks context changes.
- `coregen::singbox` ports sing-box output, including rule sets, fakeip, typed DNS server schema, Clash API/cache file, mux, TUN, bind/sendThrough, and route/DNS behavior.
- Core generation receives platform facts through explicit inputs rather than reading OS state directly.

Golden testing is the parity contract:

- Golden fixtures are exported from the read-only v2rayN reference behavior.
- Rust generation canonicalizes JSON and diffs generated output against the golden corpus.
- Golden fixtures must cover high-risk parity areas: ordinary protocols, DNS final/direct detection, TUN, platform pre-socks forwarding, and per-rule outbounds.
- Where the binary exists, generated sing-box configs must pass `sing-box check -c`.
- When the binary is not available, tests must skip acceptance with explicit evidence and still run JSON golden parity.

Config generation must use live model fields only. There is no legacy migration, no obsolete database columns, and no generator dependency on obsolete v2rayN fields.

## Consequences

- Config generator tests must assert on final sing-box JSON.
- Raw JSON is allowed only at defined rule-set boundaries; normal profile, DNS, routing, transport, and protocol data should be typed.
- Platform-specific behavior that changes config output must be injected and fixture-tested for Windows, Linux, and macOS cases.
- Later implementation batches must keep sing-box parity as the required config quality target.
