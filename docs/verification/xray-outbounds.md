# Xray Outbounds Verification

Batch `03-02-xray-coregen-outbounds` ports the deterministic Xray outbound layer from v2rayN:

- Typed serde models use Xray camelCase names and skip `None` fields to match v2rayN `JsonIgnoreCondition.WhenWritingNull`.
- Live Xray protocols are generated for VMess, VLESS, Shadowsocks, Trojan, Hysteria2, WireGuard, SOCKS, and HTTP.
- Live Xray transports are generated for raw, kcp, ws, httpupgrade, xhttp, and grpc. Current v2rayN rejects quic in the full Xray generator, so quic is not emitted in this batch.
- TLS/reality stream security includes ALPN, uTLS fingerprint, ECH list with `echForceQuery`, pinned PEM chains, pinned cert SHA-256, and reality public key fields.
- Proxy chains use `streamSettings.sockopt.dialerProxy`; xhttp `downloadSettings.sockopt.dialerProxy` is rewritten at the same time.
- Policy groups expand children into tagged outbounds and emit Xray balancers plus observatory or burstObservatory according to `MultipleLoad`.

Finalmask precedence:

1. Transport-specific masks are produced first: kcp writes UDP masks; hysteria2 writes QUIC params and optional salamander UDP mask.
2. A non-empty profile `Finalmask` replaces the transport mask.
3. Global fragment composition treats finalmask as the merge target. It adds TCP fragment only when `finalmask.tcp` is absent or empty, adds UDP noise only when `finalmask.udp` is absent or empty, and skips any outbound that already has `dialerProxy`.

Golden fixtures:

- `tests/golden/xray/outbounds/vless_tls_xhttp_fragment.json`
- `tests/golden/xray/outbounds/policy_group_least_load.json`

Verification:

```sh
cargo test -p voya-core xray_outbound --all-targets
cargo test -p voya-core policy_group --all-targets
test -f docs/verification/xray-outbounds.md
```

External `xray run -test` acceptance is not run in this batch because the repository does not yet have the runtime/core binary discovery layer. Follow-up batch `03-06-configgen-golden-gates` wires optional binary acceptance and records skip reasons when the Xray executable is missing.
