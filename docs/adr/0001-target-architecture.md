# ADR 0001: Target Architecture

Status: Accepted

Date: 2026-05-31

## Context

VoyaVPN is a greenfield rewrite of v2rayN into Tauri 2, Rust, React, TypeScript, Tailwind, and shadcn/ui. The reference app keeps business logic in `ServiceLib` with WPF and Avalonia UI projects. The rewrite must preserve behavior while removing the dual-GUI C# architecture.

Reference evidence:

- v2rayN business logic: `<v2rayN>/v2rayN/ServiceLib`
- v2rayN tests: `<v2rayN>/v2rayN/ServiceLib.Tests`
- UI references: `<v2rayN>/v2rayN/v2rayN` and `<v2rayN>/v2rayN/v2rayN.Desktop`
- Target module boundaries are captured in the decision below.

## Decision

Use a Rust workspace with these ownership boundaries:

- `crates/voya-core`: pure, OS-free, deterministic domain logic. Owns live models and enums, share-link parsers, routing/DNS logic, sing-box config generation, config canonicalization, and golden-test helpers. Clocks, random values, port allocation, filesystem reads, and platform facts are injected.
- `crates/voya-db`: fresh sqlx SQLite schema, migrations, repositories, config/default persistence, and the only typed-blob persistence boundary.
- `crates/voya-platform`: OS path resolution, system proxy, TUN/elevation, autostart, process/job handling, binary permissions, and platform adapters.
- `crates/voya-net`: HTTP downloads, subscriptions, update checks, Clash REST/WebSocket, and ruleset/geo fetches.
- `crates/voya-app`: app orchestration, managers, supervisor actor, stats manager, typed command handlers, and event dispatch. Product code names connection monitoring and traffic-mode behavior `proxy_runtime`; its network adapter remains the accurately named sing-box Clash-compatible REST/WebSocket client in `voya-net`.
- `src-tauri`: Tauri bootstrap, command/export registration, app state injection, tray, capabilities, plugins, packaging, and lifecycle glue.
- `src`: React app, shadcn/ui components, Zustand/TanStack Query state, modal stack, i18n, and typed IPC wrappers under `src/ipc`.

Implementation proceeds subsystem-by-subsystem. When feasible, each slice lands backend logic, frontend UI, tests, and IPC wiring together.

Persistence is a fresh VoyaVPN schema only. There is no v2rayN data migration path, no legacy compatibility layer, and no obsolete v2rayN columns. v2rayN `[Obsolete]` profile fields such as `HeaderType`, `RequestHost`, `Path`, `Extra`, `Ports`, `AlterId`, `Flow`, `Id`, and `Security` must not be introduced into the schema or IPC DTOs.

Profiles use strict tagged unions for protocol, transport, and TLS data. The domain types remain in `voya-core`, public camel-case DTOs remain in `voya-contracts`, and only `voya-db` may serialize the tagged domain values into SQLite `TEXT` columns.

## Consequences

- `voya-core` can be tested headlessly and must not depend on Tauri, OS APIs, sqlx pools, process state, or network clients.
- `voya-app` coordinates side effects but must not become a direct port of the C# monolith.
- Database compatibility favors correctness for the new app over migration from v2rayN installations.
- Later batches must keep generated TypeScript contracts and DB migrations aligned with these boundaries.

## Amendment (2026-07): Monorepo Path Contract

The ownership boundaries above remain accepted. The repository layout moved to a pnpm monorepo without changing those responsibilities:

- Historical `src-tauri` references now map to `apps/desktop/src-tauri`.
- Historical `src` references now map to `apps/desktop/src`.
- Historical `src/ipc/bindings.ts` references now map to `apps/desktop/src/ipc/bindings.ts`.
- Locale files now live in `packages/i18n/src/locales`.

Shared frontend code lives in source-only `packages/*` modules. Desktop-private code keeps the `@/*` alias to `apps/desktop/src`; shared imports use `@voya/*`.

## Amendment (2026-10-09): Single Database Baseline

This replaces the 2026-09-11, 2026-09-13 and 2026-10-09 database amendments.

The schema is one file, `crates/voya-db/migrations/0001_schema.sql`, and its
SQLx version is 1. Startup accepts an empty database, including an empty SQLx
bookkeeping table left by an interrupted first launch, or one whose single
migration record is successful and carries this baseline's version and
checksum. Anything else is inspected through a read-only connection and refused
before WAL checkpointing or initialization can modify the file. The error names
the failed check and carries a manual reset command; the application never
deletes or upgrades the file itself. Settings DTOs and node bundles keep their
own version 1, and settings are read strictly and never rewritten at startup.

When startup fails because the database is refused or cannot be read, the
desktop startup dialog offers Reset Database next to Quit, and the mobile
startup screen offers the same reset. Reset renames the database and its
`-wal`/`-shm` sidecars to `voyavpn-reset-<timestamp>.sqlite` in the same folder
and starts again on a fresh database. A locked database or a filesystem error
is not offered a reset.

The schema it defines:

- A node's list position is `profile_items.sort`, indexed with `index_id`.
  `profile_ex_items` holds only what was last measured; a missing row means
  not measured.
- `profile_items.config_type` is generated from the protocol blob's `kind`, so
  the two cannot disagree and a protocol without a tag cannot be stored.
- Measurements are cleared in one place, the
  `clear_country_on_connection_change` trigger.
- `profile_items`, `subscriptions`, `routing_items` and `policy_groups` record
  `created_at`, set by SQLite on insert and never written by an upsert.
- `policy_group_members` makes `(group_id, position)` unique.
- Only tables that grow with the node list carry secondary indexes.
- `subscriptions.url` is not unique: two subscriptions may fetch the same URL
  with different filters.

Until 1.0, a schema change edits this file and renames its version prefix, so
every existing database is refused once and reset; an in-place edit would keep
the version and change the checksum, which refuses them too but reports it as a
checksum mismatch. Once 1.0 ships the baseline is frozen, and a later schema
change is an incremental migration file on top of it. Startup will then accept
a database whose migration records form an unbroken, successful,
checksum-matching chain from 0001, and the migrator applies the missing tail. A
retired settings key is removed in the migration with `json_remove`, since the
settings payload is decoded strictly.

Stored credentials (node passwords and UUIDs, the self-hosted node's REALITY
private key, subscription URLs that carry tokens) remain plain JSON in the
database file. Moving them to the platform keychain is left to a separate
decision.
