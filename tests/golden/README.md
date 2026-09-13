# Golden Fixtures

Golden fixtures are the config-generation oracle for VoyaVPN. They compare VoyaVPN-generated sing-box JSON with reference JSON derived from the read-only v2rayN behavior. Golden tests assert on generated core configuration, not only on entities, DTOs, or intermediate snapshots.

## Reference Inputs

Reference behavior comes from these areas of `<v2rayN>/v2rayN/`:

- `<v2rayN>/v2rayN/ServiceLib/Handler/Builder/CoreConfigContextBuilder.cs`
- `<v2rayN>/v2rayN/ServiceLib/Services/CoreConfig/Singbox/**`
- `<v2rayN>/v2rayN/ServiceLib/Sample/**`
- `<v2rayN>/v2rayN/ServiceLib.Tests/CoreConfig/**`

The reference copy is a read-only checkout of upstream `2dust/v2rayN`. Retired features are outside the current contract; see [ADR 0008](../../docs/adr/0008-manual-node-groups.md).

## Fixture Shape

`matrix.json` is the matrix-driven index for sing-box golden cases. Each case's `fixture` path is resolved relative to `tests/golden/`, and its reference JSON is organized by generated-config area under `singbox/<area>/*.json`.

```text
tests/golden/
  README.md
  matrix.json
  singbox/
    dns/
      *.json
    inbounds/
      *.json
    outbounds/
      *.json
    route/
      *.json
```

`matrix.json` has a `version` and a `cases` array. Every case entry uses these fields:

- `id`: stable, unique case identifier used in failure output.
- `core`: target core; the current runner accepts `"sing-box"`.
- `fixture`: reference JSON path relative to `tests/golden/`, normally `singbox/<area>/<name>.json`.
- `generated`: selector for the generated section; it must be supported by `generated_value_for_case()` in `crates/voya-core/src/golden.rs`.
- `summary`: non-empty behavior description.
- `hotspots`: non-empty tags identifying the parity risks covered by the case.
- `reference_paths`: non-empty v2rayN-relative source or test paths that justify the reference behavior.
- `core_acceptance`: opts the case into binary acceptance. When it is `true`, `acceptance_configs_for_case()` in `crates/voya-core/src/golden.rs` must return at least one full config for the case, and `sing-box check -c` runs against each of them; the manifest test fails if the entry is missing.
- `volatile_fields`: JSON Pointer entries to remove from both reference and generated JSON before comparison. Each entry has a `pointer` and a non-empty `reason`.

## Adding a Fixture

1. Append a complete case entry to `tests/golden/matrix.json`, using an existing or concurrently implemented `generated` selector.
2. Place the corresponding reference JSON under the appropriate `tests/golden/singbox/<area>/` directory and point `fixture` at it.

## Canonicalization

Canonicalization makes diffs stable without hiding behavior:

- Parse fixtures as JSON and fail on invalid JSON.
- Remove only the JSON Pointer values declared in the case's `volatile_fields`, from both the reference and generated values.
- Recursively sort object keys.
- Preserve array order. Array order is behavior for outbounds, rules, DNS servers, and inbounds.
- Preserve the difference between missing fields, `null`, empty arrays, and empty objects unless the matrix entry explicitly declares a field volatile.
- Normalize numeric representation through the JSON parser.
- Pretty-print with two-space indentation and a trailing newline.
- Prefer deterministic generated inputs over ignore rules. Random ports, interface names, timestamps, temp paths, UUIDs, and generated file paths should be supplied by the test environment where possible.

Any `volatile_fields` entry must include a concrete reason and should be rare. It is not acceptable to ignore whole outbounds, rules, DNS sections, or template output to make a fixture pass.

## Required Coverage

The golden corpus should grow around these case groups:

- Basic single-node sing-box output for each supported protocol, with no selector/urltest or user-chain outbounds.
- Transport and security combinations: raw, ws, grpc, h2, httpupgrade, quic; none, tls, reality, ech; mux on and off.
- DNS: simple DNS, raw DNS override, fakeip, hosts, expected IPs, bootstrap, final DNS direct/proxy detection, TUN DNS.
- TUN and pre-socks: sing-box TUN inbound/rules, main/pre context split, loopback pre-socks behavior.
- Stats and logs: sing-box Clash API/cache file config.
- Per-rule outbounds and routing splits for direct, block, proxy, and remark-targeted generated outbounds.

## Core Acceptance

Golden JSON diffing always runs for every matrix case. Core binary acceptance is optional and additive: set `VOYA_GOLDEN_ACCEPTANCE=1` to generate the dedicated acceptance config and run:

```text
sing-box check -c <generated-config>
```

The runner first uses `VOYA_SINGBOX_BIN` when set, then searches `PATH` for `sing-box` (or `sing-box.exe`). If the binary is unavailable, it prints a skip reason while JSON parity remains authoritative for deterministic checks. Every case marked `core_acceptance` is checked, once per config returned by `acceptance_configs_for_case()`.
