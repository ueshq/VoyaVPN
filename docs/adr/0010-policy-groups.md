# ADR 0010: Policy Groups

Status: Accepted

Date: 2026-09-13

## Context

ADR 0008 reduced node groups to a display of subscription ownership, so a
connection always used exactly one node. Comparable clients let the user pick
from a set of nodes, or let the core choose the fastest or the first reachable
one. sing-box provides `selector` and `urltest` outbounds and reports their
state through its Clash-compatible API.

## Decision

- Policy groups are a separate model from ADR 0008's source groups. They live
  in `policy_groups` and `policy_group_members` in database baseline 0010.
  `app_state.active_group_id` sits next to `active_profile_id`, and a CHECK
  constraint keeps a node and a group from being active at the same time.
- Strategies: `selector` uses the stored member and otherwise the first one;
  `urltest` uses the configured probe URL, interval and tolerance within
  clamped ranges; `fallback` is a `urltest` with a 30000 ms tolerance. The
  tolerance is capped because sing-box adds it to a 16-bit delay and larger
  values wrap. Load balancing and nested groups are not offered.
- Members are the explicit nodes in their saved order, then every node of an
  optional bound subscription, without duplicates. They are resolved against
  the current node list whenever a group is listed or connected, so a
  subscription update never leaves a group pointing at stale nodes. Members the
  generator cannot use are dropped with a member-scoped warning; a group with
  no usable member fails with `PolicyGroupWithoutValidMembers` instead of
  producing an invalid configuration.
- Only the active group is generated. It carries the `proxy` tag, so the route
  final outbound, DNS detours and rule-set download detours are unchanged.
  Member tags never collide with the reserved outbound tags.
- The running group is read from the Clash API `/proxies` endpoint. A selector
  choice is stored and, while connected, applied live. The group delay test
  uses `/group/{tag}/delay`.
- The first import that brings nodes into a subscription creates a
  `"{subscription} · Auto"` lowest-latency group bound to it, when
  `behavior.autoCreateSubscriptionGroup` is on (the default). The group is
  never activated automatically and is not recreated after the user deletes
  it. Deleting the subscription deletes its import-created group; groups the
  user built survive and lose the binding.
- Surfaces: group cards and the editor on the Nodes page; a Policy groups tab
  in Network activity with the current member and delays; the Home card shows
  the active group and the member traffic goes through; a Policy Groups
  submenu in the tray once a group exists.

## Consequences

- While a group is active, traffic statistics are not attributed to a single
  node.
- There is no backend `SelectTab` target for the Policy groups tab; nothing on
  the backend needs to open it.
- ADR 0008 still governs the source-derived groups on the Nodes page; only
  policy groups take part in config generation.

## Amendment (2026-09-13)

The import-created group's suffix follows the interface language when it is
created: `· Auto`, `· 自动` or `· 自動`. Existing names are not rewritten.

## Revision 2026-09-14: Clarity on the Nodes page

- Wording is two states everywhere: "In use" while connected, "Selected"
  otherwise. The node card highlight marks the node a connection uses, not the
  row last clicked.
- Choosing a node while a group is active, or a group while a node is used on
  its own, leaves a toast naming what was set aside.
- A group created by a subscription import carries a "Created for {name}"
  badge.
- The toolbar has Test all with done/total progress, a View menu (sort by
  latency, hide unreachable nodes) and the speed test settings, which moved
  out of Settings. Collapsed groups and the view choices persist in
  `voyavpn.nodeList`. A group test still covers members hidden by the filter.
  Hiding individual subscription nodes is not offered: it needs a stored node
  field.

