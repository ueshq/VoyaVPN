# DB Schema Verification

## Source Alignment

- Planning source: `.agents/rollouts/voyavpn-full-rewrite/spec.md` and `plan.md`, batch `01-04-data-models-db-config`.
- v2rayN model references:
  - `ServiceLib/Models/Entities/ProfileItem.cs`
  - `ServiceLib/Models/Entities/ProtocolExtraItem.cs`
  - `ServiceLib/Models/Entities/TransportExtraItem.cs`
  - `ServiceLib/Models/Configs/Config.cs`
  - `ServiceLib/Models/Configs/ConfigItems.cs`
  - `ServiceLib/Enums/EConfigType.cs`
  - `ServiceLib/Enums/ECoreType.cs`

## Fresh Schema Notes

- Migration: `crates/voya-db/migrations/0001_fresh_schema.sql`.
- Tables introduced: `profile_items`, `profile_ex_items`, `subscriptions`, `routing_items`, `dns_items`, `full_config_template_items`, and `server_stat_items`.
- This is a fresh VoyaVPN schema. No v2rayN migration path or legacy compatibility columns are present.
- `profile_items` intentionally omits obsolete v2rayN profile columns: `HeaderType`, `RequestHost`, `Path`, `Extra`, `Ports`, `AlterId`, `Flow`, `Id`, and `Security`.
- `ProtocolExtraItem` and `TransportExtraItem` remain typed in `voya-core` and generated IPC. They are serialized to SQLite `TEXT` only through `voya-db::blob` into `profile_items.protocol_extra` and `profile_items.transport_extra`.
- Core entity shapes are registered with the Specta builder so `src/ipc/bindings.ts` includes generated DTOs for the current model surface instead of handwritten TypeScript.
- `EConfigType` and `ECoreType` discriminants are encoded as numeric Rust enums and stored as INTEGER columns.

## Defaults And Persistence

- `AppConfig::default()` mirrors the v2rayN foundation defaults used in `ConfigHandler.LoadConfig`: socks inbound `10808`, log level `warning`, routing domain strategy `AsIs`, TUN MTU `9000`, speed test timeout `10`, mux defaults, hysteria defaults, and builtin simple DNS defaults.
- `AppConfigStore` writes pretty JSON atomically with a temporary file and reloads defaults when the config file is absent.
- Unit and integration tests cover:
  - enum discriminants,
  - omitted obsolete profile fields,
  - typed blob serialization,
  - migrated schema column names,
  - profile persistence across a SQLite pool restart,
  - settings persistence across an `AppConfigStore` restart.

## Verification

- `cargo test -p voya-core --all-targets` - passed, 9 tests.
- `cargo test -p voya-db --all-targets` - passed, 6 tests.
- `pnpm bindings:check` - passed, generated IPC bindings are up to date.
