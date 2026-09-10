# ADR 0002: Typed IPC And Events

Status: Accepted

Date: 2026-05-31

## Context

v2rayN uses in-process ReactiveUI/MVVM calls and synchronized `EventChannel<T>` subjects in `ServiceLib/Events/AppEvents.cs` and `ServiceLib/Events/EventChannel.cs`. VoyaVPN introduces a process boundary between the Tauri/Rust backend and the TypeScript frontend, so contracts must be explicit and generated.

## Decision

Rust command inputs, command outputs, DTOs, errors, and event payloads derive `specta::Type`. `tauri-specta` exports generated TypeScript bindings to `src/ipc/bindings.ts`.

Frontend rules:

- Only `src/ipc/**` may import `@tauri-apps/api`.
- App features call typed wrappers from `src/ipc`, not raw `invoke` or `listen`.
- TypeScript IPC DTOs that mirror Rust types are generated, not hand-written.
- Generated binding drift is a build failure once the scaffold exists.

Backend command groups follow subsystem ownership: profiles, subscriptions, routing, DNS, config generation, core runtime, system proxy, TUN/elevation, proxy runtime, speedtest, updates, hotkeys, QR, and certificates. The proxy runtime is backed by the sing-box Clash-compatible API, whose protocol names remain unchanged below the product boundary.

Events use three frontend channels:

- Invalidation events: backend changes that invalidate TanStack Query data, such as profiles, subscriptions, routings, DNS, settings, and proxy runtime data.
- Transient streams: live state stored outside cached queries, such as log lines, statistics, core state, speedtest results, system proxy changes, and TUN changes.
- Imperative app events: shell actions derived from v2rayN `AppEvents`, such as reload, show/hide, add via scan, add via clipboard, shutdown, set default server, route menu refresh, test server, inbound display, and user notices.

`NoticeManager` behavior maps into imperative snack/log events. It is not a separate queueing subsystem.

Errors are exposed as a typed `AppError` union for TypeScript. Internal Rust errors may use crate-local error enums, but command boundaries convert them into the generated app-level shape.

## Consequences

- IPC is a contract generated from Rust, so TypeScript cannot drift from backend DTOs silently.
- Event names and payloads become stable integration contracts and need tests once the scaffold exists.
- Frontend features are insulated from Tauri API details and can be tested with wrapper mocks.
- Any direct `@tauri-apps/api` import outside `src/ipc` is an architectural violation.

## Amendment (2026-07): Monorepo Path Contract

The typed IPC decision remains unchanged. The monorepo migration remaps the paths used by this ADR:

- `src/ipc/bindings.ts` is now `apps/desktop/src/ipc/bindings.ts`.
- `src/ipc/**` is now `apps/desktop/src/ipc/**`.
- The lint-enforced Tauri API boundary applies to `apps/desktop/src/**/*` and `packages/**/*`; only `apps/desktop/src/ipc/**` may import `@tauri-apps/api` or Tauri plugins.
- Locale files referenced by frontend i18n now live in `packages/i18n/src/locales`.

Generated bindings are still owned by Rust `specta`/`tauri-specta` and must not be edited by hand.

## Amendment (2026-09): Retire Configurable Sources

The Settings Sources tab and its configuration-template import command have been removed. `AppSettingsV1` no longer exposes `sources`; Geo/SRS downloads and subscription conversion use the existing built-in source defaults. Conversion remains conditional on the subscription’s conversion target. Existing routing and DNS records are preserved.

The database settings reader drops the retired top-level `sources` key before strict deserialization. The schema version stays unchanged; the next settings save writes the current shape. No replacement import command or source-settings API is provided.
