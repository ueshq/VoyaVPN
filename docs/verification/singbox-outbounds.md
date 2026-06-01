# Sing-Box Outbounds Verification

Batch: `03-04-singbox-coregen-outbounds`

Implemented coverage:

- Added strict snake_case serde models for sing-box client config, inbounds, outbounds, endpoints, route rules, TLS, transports, multiplexing, and experimental `clash_api` / `cache_file`.
- Ported outbound generation for VMess, VLESS, Trojan, Shadowsocks, SOCKS, HTTP, Hysteria2, TUIC, AnyTLS, Naive, and WireGuard endpoints.
- Ported transport generation for raw HTTP, WebSocket including early-data parameters, HTTP upgrade, and gRPC.
- Ported policy group `selector` + `urltest` generation with ordered dedupe, including `Fallback` tolerance.
- Ported proxy-chain `detour` behavior for plain child nodes and group branches.
- Added null-free JSON assertions for the live protocol matrix and golden fixtures under `tests/golden/singbox/outbounds/`.

Local verification passed on 2026-05-31:

- `cargo test -p voya-core singbox_outbound --all-targets`
- `cargo test -p voya-core singbox_selector --all-targets`
- `test -f docs/verification/singbox-outbounds.md`
- Additional sweep: `cargo test -p voya-core --all-targets`

External acceptance:

- `sing-box check -c` was not run in this batch because no `sing-box` binary was available on `PATH` in the local environment. Follow-up: run the generated fixture configs through `sing-box check -c` when `VOYA_SING_BOX_BIN` or a PATH binary is available.
