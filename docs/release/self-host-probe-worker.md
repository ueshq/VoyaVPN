# Self-Hosted Node Probe Worker

The Self-hosted node page asks a small Cloudflare Worker whether a device on the
internet can open a TCP connection to the node. The Worker lives in
`apps/probe`; its design is recorded in [ADR 0011](../adr/0011-self-hosted-node.md).

## What it does

`POST /v1/probe` with `{"ports":[42443,42444]}` (one to four ports, 1024–65535).
The Worker connects back to the caller's own address (`CF-Connecting-IP`) on
each port with a 4 s timeout and answers:

```json
{ "ip": "203.0.113.7", "family": "ipv4",
  "results": [{ "port": 42443, "reachable": true, "reason": "connected", "elapsedMs": 182 }] }
```

The app sends one request per address family from a client pinned to that
family, so the IPv4 and IPv6 paths are tested separately. The response `ip` is
also the device's public address for that family.

It stores nothing, accepts no target address, refuses private and shared
caller addresses, and rate-limits per caller (6 requests a minute). Errors are
`{"error":{"code":"invalid" | "probeDenied" | "rateLimited" | "notFound" | "methodNotAllowed"}}`.
The wire contract is pinned by `tests/probe-contract/*.json`, which both the
Worker tests and `crates/voya-net/src/probe/reachability.rs` read.

## Deploy

1. Choose the production domain. The app's default is
   `https://probe.voyavpn.app` (`DEFAULT_PROBE_BASE_URL` in
   `crates/voya-net/src/probe/reachability.rs`). It must be a custom domain:
   `*.workers.dev` is unreachable from mainland China.
2. Update `routes` in `apps/probe/wrangler.jsonc` to that domain, and keep
   `workers_dev: false`.
3. Pick a `namespace_id` for the rate limit binding that is unique in the
   Cloudflare account.
4. Deploy with a wrangler you install yourself (it is deliberately not a
   workspace dependency):

   ```sh
   npx wrangler login
   pnpm --filter @voya/probe deploy
   ```

5. Verify from a machine with a known open port:

   ```sh
   curl -sS -X POST https://probe.voyavpn.app/v1/probe \
     -H 'content-type: application/json' -d '{"ports":[42443]}'
   ```

   An open port answers `"reason":"connected"`; a closed one `refused` or
   `timeout`. Repeat over IPv6 (`curl -6`).

If the domain changes, change `DEFAULT_PROBE_BASE_URL` in the same release.
For staging, point a build at another deployment with the
`VOYAVPN_PROBE_URL` environment variable.

## Known limits to confirm per deployment

- `connect()` from `cloudflare:sockets` is expected to accept a bare IPv6
  literal as `hostname`; confirm with an IPv6 caller before the first release.
- Cloudflare refuses outbound sockets to its own address ranges and to port 25,
  so a node behind Cloudflare Tunnel or WARP always probes as unreachable.
- A "reachable" answer proves the path from Cloudflare, not the route from
  mainland China.
- If the service is down, the app reports an inferred verdict and says the
  test service did not answer; nothing else depends on it.
