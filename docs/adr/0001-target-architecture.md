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

## Amendment (2026-09-11): Current Database Baseline Only

VoyaVPN initializes new databases from the single `0010_current_schema.sql`
baseline. Existing databases must have exactly its successful SQLx record and
matching checksum. Earlier migration histories, newer schemas and invalid records
are inspected through a read-only connection and rejected before WAL checkpointing
or initialization can modify the file. The error reports
the path and a manual reset hint; the application never deletes or upgrades it.
An empty database, including an empty SQLx bookkeeping table left by interrupted
initialization, can be initialized normally.

The database baseline identifier is 10; settings DTOs and node bundles retain their
current version 1. Current optional-field defaults remain supported, but there are
no historical settings conversions. Startup reads settings without rewriting them.

## Amendment (2026-09-13): Reset From the Startup Dialog

Databases are still never upgraded between baselines. When startup fails
because the database has an unsupported schema or cannot be read, the startup
dialog offers Reset Database next to Quit. Reset renames the database and its
`-wal`/`-shm` sidecars to `voyavpn-reset-<timestamp>.sqlite` in the same folder,
then relaunches the app, which creates a fresh current-baseline database. A
locked database or a filesystem error is not offered a reset.
