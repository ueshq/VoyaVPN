# ADR 0008: Source-Derived Node Groups

Status: Accepted

Date: 2026-09-11

## Decision

Nodes are ordinary protocol endpoints selected explicitly by the user. The Nodes
page derives groups solely from persisted subscription ownership: all nodes
without a subscription belong to Local nodes; each subscription has its own source
group. Grouping has no role in config generation, runtime selection, recovery,
DNS or routing.

Group keys are `subscription:<id>` and `local`; names never establish identity.
Subscription groups follow subscription `sort`, then ID, with Local nodes last.
Empty subscriptions remain visible; Local nodes appears only when it contains
nodes. Subscription-owned nodes retain their source group even if subscription
information is temporarily unavailable. Source groups are not persisted as
another group model.

The page flattens headers and expanded nodes into one virtual list with continuous
bordered panels. All groups start expanded; page-local collapses survive query
refresh and subscription renames. Node IDs identify nodes regardless of duplicate
names. The page has no search or group editor. Adding a node opens its editor
directly. Group creation, renaming, deletion, ordering and membership assignment
are unsupported. Subscription settings, update and deletion remain supported;
source group names follow subscription names.

Only local nodes can be edited, individually deleted or reordered. Local ordering
includes all local nodes, independent of retired manual memberships. Nodes cannot
be copied into new saved nodes or moved between groups. Subscription nodes can
be selected, connected, inspected, tested and exported. Group tests include all
members, and group export resolves current complete membership independently of
collapsed or virtualized rows. Share-link copying remains supported.

Public mutations check **persisted** ownership before writing any member of a
batch. A typed `SubscriptionReadOnly` validation code crosses IPC. The subscription
updater has a separate internal write path; manual import rejects a subscription
destination. Matching nodes in local storage or another subscription never get
adopted by an update. Untrusted imported IDs cannot overwrite another source's
node. Imports from clipboard text, QR images and screen scans create local nodes.

Only the node's Use action selects and connects/restarts it. Imports, latency tests
and ordering never choose an exit. Removing the selected node clears selection
and stops its runtime; deleting a subscription also deletes its nodes and
reconciles the connection. Recovery of the same surviving node remains supported.
Updates and imports do not automatically select a replacement node or connect.
Runtime connection monitoring is owned by Network activity, not Nodes.

## Persistence and IPC

This decision supersedes manual node groups and their membership model. Manual
group commands, DTOs, repositories and invalidation scope, as well as the saved
node copy command, are removed. The UI derives groups from the existing profiles
and subscriptions queries and their invalidation events.

The `0009_current_schema.sql` baseline and its checksum remain unchanged, so
current databases continue opening without a reset. The `node_groups` and
`node_group_memberships` tables remain inert baseline storage; application code
no longer reads or writes them. Existing locally owned nodes automatically appear
under Local nodes without changing their IDs, content or selection. SQLite's
existing foreign-key cleanup remains in effect when a node is deleted. Earlier
historical databases remain rejected unchanged under ADR 0001; no migration or
legacy compatibility layer is added.

## Retired capabilities

Executable policy groups, proxy chains and user-supplied complete sing-box
configurations are unsupported. Their protocol variants, configuration branches,
editors, runtime selection/testing commands and subscription auto-group settings
are removed without aliases. There is no conversion of retired profiles or groups.

This narrows ADR 0003's parity boundary to supported ordinary protocols, per-node
routes, DNS, traffic modes, core monitoring and platform TUN. Platform-internal
pre-SOCKS forwarding is retained; it is not a user proxy chain. Protocol/node
bundle import and share-link export remain supported. Base64 share-link and Voya
node-bundle export were removed on 2026-09-13; existing Voya bundles still import.
Client configuration export has been removed globally; runtime sing-box config
generation remains supported. Full configuration import is rejected.

## Amendment (2026-09-13): Policy Groups

Source-derived groups still play no role in config generation. Policy groups
(ADR 0010) are a separate, stored model that does: the active policy group is
what connecting uses instead of a single node.

## Amendment (2026-09-13): One Add menu, and subscription links update at once

- The Nodes toolbar has one Add menu and an Update all subscriptions button.
  The menu lists, in order, Paste links or subscription URLs, Import from
  clipboard, Scan screen, Add subscription, Enter a node manually and New
  policy group. The separate Import menu is gone; scanning a QR image lives in
  the paste dialog and appends to what was typed.
- A subscription URL in imported text still only creates the source, but
  `ImportProfilesResult.addedSubscriptionIds` names it and the page updates it
  straight away, so pasting a provider link brings its nodes in.
- Import line codes are typed (`unsupportedProtocol`, `missingField`,
  `invalidPort`, `parseFailed`); the untranslated parse diagnostic is no longer
  shown.
- Deleting the node or group the core is running, directly or through a
  subscription update, raises the `activeSelectionRemoved` notice. The delete
  confirmation says so beforehand when the running node is included.
- A manual subscription update always runs; `enabled` only switches automatic
  updates.

## Amendment (2026-09-13): Imports, updates and removals speak up

- A subscription URL in imported text still only creates the source, but the
  import result now carries `addedSubscriptionIds` and the Nodes page updates
  each one straight away, so pasting a provider link brings its nodes in.
- A manual subscription update runs whether or not automatic updates are
  switched on; `enabled` only gates the scheduler.
- When deleting a node, a policy group or a subscription, or updating a
  subscription, stops the running connection, the flow raises
  `NoticeCode::ActiveSelectionRemoved`. The node delete confirmation says in
  advance when the running node is among the selection.
- Share-link parse failures cross IPC as `unsupportedProtocol`,
  `missingField`, `invalidPort` or `parseFailed`; the untranslated diagnostic
  is kept for logs and not shown.
- An import-created policy group is named with the suffix of the interface
  language (" · Auto", " · 自动", " · 自動").
