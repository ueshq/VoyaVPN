# Context Builder Verification

Batch `03-01-context-builder` ports the shared Xray and sing-box context resolution layer into pure `voya-core`.

Implemented checks:

- `CoreGenEnv` injects profiles, subscriptions, DNS items, full-config templates, routing, local ports, virtual IDs, and platform facts.
- `CoreConfigContextBuilder` resolves active nodes, subscription-level virtual proxy chains, group and proxy-chain children, per-rule outbound nodes, template inputs, DNS inputs, and pre-socks contexts.
- Protected domains are gathered deterministically from node addresses, ECH query SNI, and xhttp `downloadSettings.address`.
- Group traversal detects cycles and dedupes child IDs while preserving first-seen order.
- `build_all` disables TUN on the main context when a pre-socks context is built, then merges protected domains.

Verification:

- `cargo test -p voya-core context --all-targets`
- `test -f docs/verification/context-builder.md`
