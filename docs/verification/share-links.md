# Share Links Verification

Batch: `02-03-share-link-parsers`

## Scope

- Added `voya_core::fmt` with a `ShareFmt` trait and per-protocol parse/export implementations.
- Covered share protocols: VMess, VLESS, Trojan, Shadowsocks, SOCKS, Hysteria2/`hy2`, TUIC, WireGuard, AnyTLS, and Naive `naive+https`/`naive+quic`.
- Added shared query/stream handling for `security`, `sni`, `alpn`, `fp`, `pbk`, `sid`, `spx`, `pqv`, `ech`, `pcs`, `fm`, and transport parameters for raw, KCP, WS, HTTP upgrade, XHTTP, and gRPC.
- Added `v2rayn://` inner import/export helpers with safe imported ID rewriting and group child reference remapping.
- Added pure full-config import classifiers for Xray, sing-box, mihomo/Clash, and Hysteria2 custom configs. `voya-core` does not write temp files; callers receive content, extension, core type, and a `Custom` profile shell.
- Added SIP008 Shadowsocks and WireGuard config-file helpers.

## v2rayN Parity Notes

Reference files read:

- `ServiceLib/Handler/Fmt/BaseFmt.cs`
- `ServiceLib/Handler/Fmt/FmtHandler.cs`
- `ServiceLib/Handler/Fmt/{Vmess,VLESS,Trojan,Shadowsocks,Socks,Hysteria2,Tuic,Wireguard,Anytls,Naive,Inner,V2ray,Singbox,Clash}.cs`
- `ServiceLib.Tests/Fmt/{FmtHandlerTests,WireguardFmtTests,InnerFmtTests}.cs`

Ported behavior:

- VMess keeps v2rayN base64 JSON export and standard URI import.
- VLESS, Trojan, AnyTLS, and Naive use the shared BaseFmt stream query codec.
- Shadowsocks handles legacy base64, SIP002, `obfs-local`, `simple-obfs`, and `v2ray-plugin` plugin fields.
- Hysteria2 exports `hysteria2://` and imports both `hysteria2://` and `hy2://`.
- WireGuard share links and `[Interface]`/`[Peer]` config parsing match the v2rayN test coverage for inline comments and IPv6 endpoints.
- Inner format accepts v2rayN `ProtoExtraObj`/`TransportExtraObj` payloads while exporting Voya's typed model as compatible inner object fields.

Intentional Voya differences:

- Full custom config helpers are pure and return content to the caller instead of writing a temporary file in `voya-core`.
- Inner import ID rewriting is deterministic inside the helper rather than using a process-wide random session salt.

## Verification Commands

- `cargo test -p voya-core fmt --all-targets`
- `cargo test -p voya-core share --all-targets`

Local result on 2026-05-31: both commands passed. A broader `cargo test -p voya-core --all-targets` pass also completed with 19 tests passing.
