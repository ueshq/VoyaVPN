# ADR 0014: Node IPv6 Egress Detection

Status: Accepted

Date: 2026-09-24

## Context

ADR 0009 (revision 2026-09-24) turned the TUN IPv6 switch on by default so
that direct routes keep native IPv6. Most proxy nodes have no IPv6 egress,
though. With IPv6 on, an IPv6 destination sent to such a node dies after the
local TCP handshake, and DNS answers from the remote resolver hand clients
exactly those destinations. The 2026-09-23 revision of ADR 0009 fixed that by
turning IPv6 off everywhere, which is what broke WeChat on a network with
native IPv6.

The switch alone cannot express "IPv6 on, but not through this node". The
node's capability has to be known.

## Decision

- After a connection settles, the core flow asks its host to run
  `CoreFlow::check_ipv6_egress` in the background
  (`CoreFlowSink::request_ipv6_egress_check`). It is asked only for reasons
  that can change the node or the switch: connect, restart, node or group
  change, TUN settings, settings save. The desktop spawns it on the Tauri
  runtime; the mobile host spawns it on its own runtime through a weak
  reference to its state. Both build a fresh flow, because the reconnect it
  may trigger needs the flow lock the settling connect still holds.
- The check does nothing while IPv6 is off. It probes the running core's
  `proxy` outbound through the Clash API URL test: first the speedtest URL as
  an IPv4 control, then three HTTPS pages on hosts that publish only AAAA
  records (`ipv6.icanhazip.com`, `api6.ipify.org`, `v6.ident.me`) in
  parallel. Any IPv6 page loading means supported; control loading and no
  IPv6 page loading means unsupported; a failed control means no answer.
  - HTTPS, because some nodes answer plain HTTP themselves, so even an
    unroutable documentation address "loads".
  - Host names, because the node resolves them, which is what proves its
    egress; an IPv6 literal over HTTPS fails the TLS handshake in sing-box's
    URL test regardless of the node.
- For a running policy group the answer is recorded against the member the
  group is using at that moment. The generator treats the group as limited
  when any member is recorded unsupported, because the group can switch
  members without regenerating the config.
- The answer is kept per node in `node-ipv6-egress.json` in the config
  directory, with the node's protocol, address and port, so a node edited to
  point elsewhere is unknown again. It is not a database column: the database
  keeps a single baseline schema, and changing it resets every install. A
  damaged file counts as empty.
- The answer is applied only if the same core is still running (same Clash API
  secret) and it differs from the record. Unknown to supported changes
  nothing. Anything to unsupported shows `NodeIpv6Unsupported` (also as a
  system notification while the window is hidden) and reconnects with
  `CoreFlowReason::Ipv6EgressChanged`, which does not ask for another check.
  Unsupported to supported shows `NodeIpv6Restored` and reconnects.
- `CoreGenEnv::ipv6_unsupported_nodes` carries the record into config
  generation, which derives `Ipv6Mode::DirectOnly` (ADR 0009).

## Consequences

- The IPv6 switch stays the user's. Detection never turns it off; it only
  narrows IPv6 to direct routes while a limited node is in use, and lifts that
  when the node is seen with IPv6 again.
- A limited node costs one extra reconnect the first time it is used, and
  whenever its answer changes. The check itself is four URL tests through the
  node, at most about ten seconds, in the background.
- A node whose outbound resolves names locally (WireGuard) is probed the same
  way; with IPv6 on, its resolver is dual-stack, so the answer still reflects
  the tunnel.
- Exported client configs do not read the record: they are for another device.
