# ADR 0008: Manual Node Groups

Status: Accepted

Date: 2026-09-11

## Decision

Nodes are ordinary protocol endpoints selected explicitly by the user. Manual groups organize saved nodes offline; they have no role in config generation, runtime selection, recovery, DNS or routing.

`voya-contracts` owns `NodeGroup`, membership, assignment-delta and snapshot DTOs. `voya-db` persists groups separately from profiles. A primary key on the membership's profile ID enforces one group per node; foreign keys remove membership when either endpoint is deleted. Removing a group keeps its nodes. Group mutations use the existing configuration UnitOfWork and invalidation channel. Membership dialogs submit only changed assignments in one transaction, preserving unrelated memberships.

The Nodes page flattens group headers and expanded nodes into one virtual list, with unassigned nodes last. Expansion is page-local. Search temporarily expands matching groups without rewriting expansion state. Node IDs identify nodes regardless of duplicate names or group moves. Node sorting stays within the node's group; group order is independent of subscription provenance.

Only the node's Use action selects and connects/restarts it. Imports, latency tests, group edits and ordering never choose an exit. Copies inherit membership; subscription refresh retains membership for surviving profile IDs; deduplication retains the canonical survivor's membership. Removing the selected node clears selection and stops its runtime. Recovery of the same surviving node remains supported. Runtime connection monitoring is owned by Network activity, not Nodes.

## Retired capabilities and upgrade

Executable policy groups, proxy chains and user-supplied complete sing-box configurations are unsupported. Their protocol variants, configuration branches, editors, runtime selection/testing commands and subscription auto-group settings are removed without aliases. Migration 0005 deletes those profiles, dependent metrics/traffic/memberships, routes that only resolve to removed profiles, their current selection, and retired settings. Ordinary nodes, valid routes and unrelated settings survive. Old groups are not converted into manual groups; existing ordinary nodes start unassigned.

This narrows ADR 0003's parity boundary to supported ordinary protocols, per-node routes, DNS, traffic modes, core monitoring and platform TUN. Platform-internal pre-SOCKS forwarding is retained; it is not a user proxy chain. Protocol/node bundle import and ordinary config export remain supported; full configuration import is rejected.
