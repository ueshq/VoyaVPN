# DNS Settings Verification

Batch: `05-02-dns-settings`

Implemented:
- SQLite `dns_items` repository, DNS manager, typed IPC commands, and runtime context loading.
- Simple DNS controls for direct, remote, bootstrap, hosts, expected IPs, serve stale, parallel query, binding-query block, and FakeIP/global FakeIP.
- Per-core advanced raw DNS editors for Xray and sing-box, using CodeMirror JSON editors.
- Typed validation issues for invalid Xray JSON, sing-box typed server schema errors, hosts, and expected IPs.
- Xray and sing-box generator tests for raw DNS override behavior in addition to existing fakeip, hosts, expected IPs, strategy, and routing DNS golden coverage.

Verification:
- `cargo test -p voya-core dns --all-targets`
- `cargo test -p voya-app dns --all-targets`
- `pnpm typecheck`
- `pnpm test -- --run`
- `test -f docs/verification/dns.md`

External checks:
- No external core binary acceptance check was run in this batch; the required DNS verification commands are deterministic unit and frontend checks.
