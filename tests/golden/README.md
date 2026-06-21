# Golden Fixtures

Golden fixtures are the config-generation oracle for VoyaVPN. They compare VoyaVPN-generated Xray and sing-box JSON against JSON exported from the read-only v2rayN reference behavior. Golden tests must assert on generated core configs, not only on entities, DTOs, or intermediate snapshots.

## Reference Inputs

Reference behavior comes from:

- `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib/Handler/Builder/CoreConfigContextBuilder.cs`
- `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib/Services/CoreConfig/V2ray/**`
- `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib/Services/CoreConfig/Singbox/**`
- `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib/Sample/**`
- `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib.Tests/CoreConfig/**`

The reference repository is read-only. If an export harness is needed later, add it in VoyaVPN or run it out-of-tree without modifying the reference source.

## Fixture Shape

Use one directory per case:

```text
tests/golden/
  README.md
  cases/
    xray-vmess-ws-tls/
      manifest.json
      input.json
      xray.reference.json
    singbox-proxy-chain-detour/
      manifest.json
      input.json
      singbox.reference.json
    dual-policy-group-observatory/
      manifest.json
      input.json
      xray.reference.json
      singbox.reference.json
```

`manifest.json` records:

- `id`: stable case ID, matching the directory name
- `summary`: short behavior description
- `cores`: `["xray"]`, `["sing-box"]`, or both
- `platform`: `generic`, `windows`, `linux`, or `macos` when output depends on platform facts
- `reference_paths`: v2rayN files or tests that justify the case
- `hotspots`: hotspot tags such as `finalmask`, `policy-group`, `proxy-chain`, `dns`, `tun`, `stats`, `template`, or `pre-socks`
- `core_acceptance`: whether Xray or sing-box binary acceptance should be attempted when binaries are discoverable
- `volatile_fields`: fields ignored during canonicalization, with a reason for each entry

`input.json` is the normalized VoyaVPN test seed: profiles, settings, routing, DNS, full-config template data, and injected platform facts. It is not the golden assertion target. The assertion target is the generated core JSON compared with `*.reference.json`.

`xray.reference.json` and `singbox.reference.json` are canonical v2rayN outputs for the same input case. Reference files should be valid JSON with no comments.

## Canonicalization

Canonicalization must make diffs stable without hiding behavior:

- Parse as JSON and fail on invalid JSON.
- Recursively sort object keys.
- Preserve array order. Array order is behavior for outbounds, rules, DNS servers, policy selectors, balancers, and inbounds.
- Preserve string values exactly except for normalized line endings.
- Preserve the difference between missing fields, `null`, empty arrays, and empty objects unless the manifest explicitly declares a field volatile.
- Normalize numeric representation through the JSON parser.
- Pretty-print with two-space indentation and a trailing newline.
- Prefer deterministic injected inputs over ignore rules. Random ports, interface names, timestamps, temp paths, UUIDs, and generated file paths should be supplied by the test environment where possible.

Any `volatile_fields` entry must include a concrete reason and should be rare. It is not acceptable to ignore whole outbounds, rules, DNS sections, or template output to make a fixture pass.

## Required Coverage

The initial golden corpus should grow around these case groups:

- Basic single-node Xray and sing-box output for each supported protocol.
- Transport and security combinations: raw, ws, grpc, xhttp, h2, kcp, httpupgrade, quic; none, tls, reality, ech; mux on and off.
- Xray finalmask: raw imported finalmask, KCP, hysteria/quic, fragment/noise merge, and chained-outbound exclusion.
- Policy groups: every `EMultipleLoad` mode, child deduplication, selector ordering, Xray observatory or burst-observatory, sing-box selector/urltest.
- Proxy chains: 2-hop, 3-hop, mixed chain/group branches, subscription `PrevProfile`/`NextProfile`, Xray `dialerProxy`, sing-box `detour`.
- DNS: simple DNS, raw DNS override, fakeip, hosts, expected IPs, bootstrap, final DNS direct/proxy detection, TUN DNS.
- TUN and pre-socks: Xray TUN inbound, sing-box TUN inbound/rules, main/pre context split, loopback pre-socks behavior.
- Stats and logs: Xray API/dokodemo stats config and sing-box Clash API/cache file config.
- Full config templates: add-proxy-only, proxy-detour, and separate `TunConfig` template output.
- Per-rule outbounds and routing splits for direct, block, proxy, and remark-targeted generated outbounds.

The reference tests already cover important examples such as generated basic proxy configs, policy group expansion, proxy chain `dialerProxy`/`detour`, combined group/chain output, routing split rules, TUN with loopback pre-socks, bind interface, and context cycle handling. The golden corpus should keep those behaviors and then expand them into full generated JSON fixtures.

## Core Acceptance

Golden JSON diffing always runs. Core binary acceptance is optional and additive:

- Xray: `xray run -test -config <generated-config>`
- sing-box: `sing-box check -c <generated-config>`

The runner should discover binaries from `VOYA_XRAY_BIN`, `VOYA_SING_BOX_BIN`, future test config, the app binary directory, then `PATH`. If a binary is missing, record a skip reason and keep the JSON parity result authoritative for local deterministic checks.

External core binaries are not required for this batch, and GPL/AGPL cores must not be bundled into installers by default.
