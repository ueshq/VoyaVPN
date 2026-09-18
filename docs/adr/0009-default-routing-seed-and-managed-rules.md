# ADR 0009: Default Routing Seed and Managed Rules

Status: Accepted

Date: 2026-09-13

## Context

A fresh install had no routing rules, so every destination went through the
proxy until the user wrote `geosite:cn` style rules by hand. ADR 0006 injected a
hardcoded AI-service domain list ahead of the user's rules, with no way to edit
or disable it. Comparable clients ship a region preset and quick switches for
blocking ads and bypassing the LAN.

## Decision

- `voya_core::routing_seed` owns the default rule set, in evaluation order:
  AI services through the proxy, QUIC to UDP 443 blocked, advertising domains
  blocked (seeded disabled), Chinese public DNS direct, private destinations
  direct, and Chinese domains and addresses direct. Anything else falls
  through to the proxy.
- `voya-app` seeds that set as an active routing profile at startup, only when
  the database has no routing profile at all. Existing profiles are never
  touched. `reset_routing_rules` restores the default set on a chosen profile
  and keeps its per-app proxy rule first.
- Managed rules are identified by reserved remarks (`voya:*`). The Rules page
  shows their translated names with a Managed badge and locks the remarks in
  the rule editor. Every rule, managed or not, has an in-row switch that flips
  its `enabled` flag, so position and edits survive.
- The generator no longer injects the AI-service list. It is ordinary routing
  data now, which supersedes ADR 0006.
- The rule IP resolution mode (`AsIs`, `IPIfNonMatch`, `IPOnDemand`) stays the
  global `routing.domainStrategy` setting, but no screen edits it; new installs
  keep `AsIs`.

## Consequences

- Upgraded installs keep their routing profiles unchanged and get the AI list
  only by restoring the default rules.
- The AI-service DNS rule follows the proxy DNS strategy like any other rule;
  it is no longer forced to `ipv4_only` under TUN without IPv6.
- `tests/golden/singbox/route/default_seed.json` pins the generated route and
  DNS rules of the seed and is accepted by `sing-box check`.

## Revision 2026-09-13: Rules page rework

- The Rules page edits only the active routing profile. The profile list and
  the create, edit, activate and delete profile actions are gone from the UI;
  the IPC commands and the table stay.
- The quick-rules bar is gone. Block ads is seeded disabled so its row and
  switch always exist; upgraded profiles without it get it by restoring the
  default rules.
- Rules reorder by drag and drop (`@dnd-kit`, its own `vendor-dnd` chunk) and
  through a row menu with top, up, down and bottom.
- When the per-app proxy rule is pinned first, the page shows it as a summary
  card above the list and edits it only through its dialog.
- The rule editor offers built-in outbounds and node names instead of free
  text, TCP and UDP checkboxes instead of a network string, hides `kind` and
  inbound tags (carried through unchanged), and rejects a rule with no matcher,
  which the generator would skip.
- While the traffic mode is global the page says the rules are skipped and
  offers to switch back to smart routing.

## Revision 2026-09-13: Per-app rules follow the platform

- The macOS NetworkExtension tunnel cannot match traffic by process, so the
  per-app proxy card, its dialog, the Settings shortcut and the rule editor's app
  field are not offered there. Stored app conditions are kept and marked as
  unsupported in the rule list. Windows and Linux are unchanged.
- The Home and Rules pages call the rule-based traffic mode "Rule", matching
  the tray.

## Revision 2026-09-14: Group outbounds, presets and repair

- A rule's outbound may be `group:<id>`. The generator resolves the group's
  members through `CoreGenEnv::get_policy_group`; the active group maps to the
  main `proxy` tag, any other group is emitted once as its own selector or
  urltest outbound tagged `"{name} [{id prefix}]"` with members tagged
  `"{group tag} / {node tag}"`. A missing group fails the build like a missing
  node; a group without usable members warns.
- On macOS the generator drops process conditions (NetworkExtension cannot
  match them) and keeps the rest of the rule.
- The rule editor offers policy groups as outbounds and one-click common rule
  sets (`geosite:cn`, `geosite:google`, `geosite:category-ads-all`, `geoip:cn`,
  `geoip:private`). A rule whose node or group no longer exists shows a
  "Use proxy" repair. Managed rules are labelled "Built-in".
- While connected, the page says that rule changes briefly reconnect; a rule's
  switch shows a spinner while its save is in flight.
- Network activity names the matched rule in the Rules page's words: the core
  reports only generated conditions, so the first enabled rule whose matcher
  tokens appear in them is taken to be the one, and the raw text stays below.

