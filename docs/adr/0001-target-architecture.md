# ADR 0001: Target Architecture

Status: Accepted

Date: 2026-05-31

## Context

VoyaVPN is a greenfield rewrite of v2rayN into Tauri 2, Rust, React, TypeScript, Tailwind, and shadcn/ui. The reference app keeps business logic in `ServiceLib` with WPF and Avalonia UI projects. The rewrite must preserve behavior while removing the dual-GUI C# architecture.

Reference evidence:

- v2rayN business logic: `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib`
- v2rayN tests: `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib.Tests`
- UI references: `/Users/afu/Dev/refs/v2rayN/v2rayN/v2rayN` and `/Users/afu/Dev/refs/v2rayN/v2rayN/v2rayN.Desktop`
- target subsystem map: `docs/source-inventory.md`

## Decision

Use a Rust workspace with these ownership boundaries:

- `crates/voya-core`: pure, OS-free, deterministic domain logic. Owns live models and enums, share-link parsers, routing/DNS logic, Xray and sing-box config generation, config canonicalization, and golden-test helpers. Clocks, random values, port allocation, filesystem reads, and platform facts are injected.
- `crates/voya-db`: fresh sqlx SQLite schema, migrations, repositories, config/default persistence, and the only typed-blob persistence boundary.
- `crates/voya-platform`: OS path resolution, system proxy, PAC, TUN/elevation, autostart, hotkeys, process/job handling, binary permissions, and platform adapters.
- `crates/voya-net`: HTTP downloads, subscriptions, update checks, Clash REST/WebSocket, WebDAV, ruleset/geo fetches, and preset network fetches.
- `crates/voya-udptest`: SOCKS5 UDP-associate channel and UDP test modes.
- `crates/voya-app`: app orchestration, managers, supervisor actor, stats manager, typed command handlers, and event dispatch.
- `src-tauri`: Tauri bootstrap, command/export registration, app state injection, tray, capabilities, plugins, packaging, and lifecycle glue.
- `src`: React app, shadcn/ui components, Zustand/TanStack Query state, modal stack, i18n, and typed IPC wrappers under `src/ipc`.

Implementation proceeds subsystem-by-subsystem. When feasible, each slice lands backend logic, frontend UI, tests, and IPC wiring together.

Persistence is a fresh VoyaVPN schema only. There is no v2rayN data migration path, no legacy compatibility layer, and no obsolete v2rayN columns. v2rayN `[Obsolete]` profile fields such as `HeaderType`, `RequestHost`, `Path`, `Extra`, `Ports`, `AlterId`, `Flow`, `Id`, and `Security` must not be introduced into the schema or IPC DTOs.

`ProtocolExtraItem` and `TransportExtraItem` are typed Rust structs across app and IPC boundaries. They may serialize to SQLite TEXT only inside `voya-db` blob helpers.

## Consequences

- `voya-core` can be tested headlessly and must not depend on Tauri, OS APIs, sqlx pools, process state, or network clients.
- `voya-app` coordinates side effects but must not become a direct port of the C# monolith.
- Database compatibility favors correctness for the new app over migration from v2rayN installations.
- Later batches must keep generated TypeScript contracts and DB migrations aligned with these boundaries.
