# Routing Verification

Batch: `05-01-routing-settings-editor`

Implemented checks:

- `voya-db` persists `RoutingItem` rows with JSON-serialized `RulesItem` arrays at the blob boundary.
- `voya-app` routing manager creates, edits, activates, deletes, imports templates, and mutates rules.
- Runtime config generation loads active routing from SQLite before starting a core.
- Routing IPC mutations emit `routings` invalidation events and restart the core when it is already connected.
- The frontend Routing tab uses typed IPC wrappers from `src/ipc` for profile CRUD, rule CRUD, template import, domain strategy edits, and activation.
- `voya-core` tests assert that routing rules serialize into both generated Xray and sing-box JSON.

Local verification:

- `cargo test -p voya-app routing --all-targets`
- `cargo test -p voya-core routing --all-targets`
- `pnpm typecheck`
- `pnpm test -- --run`
- `test -f docs/verification/routing.md`

External template fetches are covered through the `RouteRulesTemplateSourceUrl` command path. Real network template availability was not pinned in this batch because the upstream preset URLs are external services; failures fall back to built-in templates and are logged.
