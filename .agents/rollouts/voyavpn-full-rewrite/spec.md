# VoyaVPN Full Rewrite Specification

## 1. Executive Summary

- Initiative type: greenfield rewrite and architecture convergence.
- Primary target repo: `/Users/afu/Dev/refs/VoyaVPN`.
- Read-only reference repo: `/Users/afu/Dev/refs/v2rayN/v2rayN`.
- Primary decision: rebuild v2rayN as VoyaVPN with Tauri 2, Rust, React, TypeScript, Tailwind, and shadcn/ui.
- Definition of success: VoyaVPN reaches full functional parity with the current v2rayN business logic and UI surface while replacing the C# dual-GUI architecture with a single typed Rust and TypeScript stack, a deterministic Rust core generation layer, generated IPC bindings, golden parity tests against v2rayN, and packageable builds for Windows, Linux, and macOS.

## 2. Problem And Current State

### 2.1 Problem Statement

v2rayN is mature but split across a C# ServiceLib, WPF UI, Avalonia UI, and multiple platform-specific flows. The rewrite must preserve behavior while moving to a clean architecture where core business logic is testable headlessly, UI and backend contracts are generated from one source of truth, and OS-specific behavior is isolated.

### 2.2 Current State

- Existing code paths or systems:
  - `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib` contains the source business logic to port.
  - `/Users/afu/Dev/refs/v2rayN/v2rayN/ServiceLib.Tests` contains current parser and config generator test patterns.
  - `/Users/afu/Dev/refs/v2rayN/v2rayN/v2rayN` contains WPF UI behavior and dialogs.
  - `/Users/afu/Dev/refs/v2rayN/v2rayN/v2rayN.Desktop` contains Avalonia UI behavior.
- Known pain points:
  - Dual UI stacks and dual runtime assumptions increase drift.
  - Config generation is high-fidelity and difficult to validate without golden tests.
  - Cross-platform behavior is distributed through many conditional paths.
  - Current IPC-style contracts do not exist because the app is in-process C# MVVM.
- External systems or dependencies:
  - Xray, sing-box, mihomo, and the other supported cores.
  - GitHub release feeds for app, core, geo, and ruleset updates.
  - OS proxy, TUN, autostart, hotkey, signing, and notarization systems.
- Existing contracts that must be preserved:
  - Share-link import and export semantics.
  - Core config output behavior for Xray and sing-box.
  - Enum integer discriminants for profile and core types.
  - User-visible workflows across profiles, subscriptions, routing, DNS, system proxy, TUN, speedtest, updates, backup, logs, Clash views, and tray.
- Known operational constraints:
  - Support Windows, Linux, and macOS from the start.
  - Start from a fresh SQLite schema with no legacy migration.
  - Do not modify the v2rayN reference repository.
  - Do not redistribute GPL or AGPL core binaries by default; use first-run or on-demand download.

### 2.3 Baseline Evidence

- Inventory source: active planning document `/Users/afu/.claude/plans/typescript-shadcn-tauri-silly-marble.md`.
- Current architecture: ServiceLib plus two C# GUI projects.
- Known hot spots:
  - `Handler/Builder/CoreConfigContextBuilder.cs`.
  - `Services/CoreConfig/V2ray/**`.
  - `Services/CoreConfig/Singbox/**`.
  - `Handler/Fmt/**`.
  - `CoreManager`, `CoreInfoManager`, `ProcessService`, `StatisticsManager`, `SysProxy`, `PacManager`, `SubscriptionHandler`, and view models.
- Assumptions to validate:
  - Required dev tools are available locally: Rust, pnpm, Node, Tauri prerequisites, sqlx tooling where needed.
  - Core binary validation commands can be run locally or guarded in CI where binaries are installed.

## 3. Goals, Non-Goals, And Success Metrics

### 3.1 Goals

- Deliver a greenfield VoyaVPN app with Rust crates for core logic, DB, platform, net, UDP tests, and app orchestration.
- Deliver a Tauri 2 shell with generated specta and tauri-specta TypeScript bindings.
- Deliver a React and TypeScript frontend using Tailwind v4, shadcn/ui, Zustand, TanStack Query, TanStack Table, Radix dialogs, i18next, and typed IPC wrappers.
- Port all live data models without obsolete columns.
- Port both Xray and sing-box config generation with golden parity.
- Port share-link parsers, profile CRUD, subscriptions, system proxy, TUN, statistics, speedtest, Clash API, updates, backup, WebDAV, autostart, hotkeys, QR generation, i18n, theming, and packaging.
- Keep all three supported platforms in scope from the first scaffold.

### 3.2 Non-Goals

- No migration from existing v2rayN user data.
- No compatibility with obsolete database columns.
- No hand-written TypeScript IPC contracts outside generated bindings and thin wrappers.
- No broad edits to the v2rayN reference repository.
- No production redistribution of GPL or AGPL core binaries without separate legal approval.
- No multi-window dialog architecture in the frontend; dialogs use the modal stack.

### 3.3 Success Metrics

- Rust workspace builds and tests pass for all crates.
- Generated `src/ipc/bindings.ts` has no drift after regeneration.
- Golden config output matches v2rayN for the configured fixture matrix.
- Xray and sing-box acceptance checks pass for generated fixture configs where binaries are available.
- `pnpm tauri dev` can add a real server, connect, stream logs, show stats, and route traffic through local inbounds.
- System proxy and TUN smoke tests pass on Windows, Linux, and macOS.
- Signed or locally packageable installers are produced for the target OS matrix.

## 4. Principles And Target State

### 4.1 Design Principles

- Keep `voya-core` pure, OS-free, deterministic, and golden-testable.
- Keep platform-specific behavior in `voya-platform`.
- Keep Tauri-specific logic in `src-tauri`.
- Use generated Rust-to-TypeScript contracts as the single source of IPC truth.
- Build subsystem by subsystem, backend plus frontend, in dependency order.
- Assert on generated core configs, not only entity snapshots.
- Prefer typed structures and explicit serde behavior over loose JSON maps except at defined template or raw config boundaries.

### 4.2 Target State

- One cross-platform app replacing the WPF and Avalonia UI split.
- Rust workspace:
  - `crates/voya-core`
  - `crates/voya-db`
  - `crates/voya-platform`
  - `crates/voya-net`
  - `crates/voya-udptest`
  - `crates/voya-app`
  - `src-tauri`
- Frontend:
  - `src` React app with typed IPC wrappers, event bridge, app shell, modal stack, server table, status bar, and all dialogs.
- Persistence:
  - Fresh SQLite schema plus JSON config defaults.
  - Typed protocol and transport extra structs serialized to SQLite only at the DB blob boundary.
- Compatibility stance:
  - Behavioral parity with current v2rayN workflows and generated configs.
  - No data migration from existing installations.

## 5. Capability Slices

### 5.1 Foundation And Shell

- Trigger: starting the VoyaVPN repo from an empty directory.
- Happy path: workspace, Tauri, React, shadcn/ui, i18n, theming, IPC demo, tracing, app shell, tray, and CI baseline compile.
- Edge cases: missing toolchains, platform-specific Tauri prerequisites, generated binding drift.
- Acceptance notes: empty app is navigable, themed, and can round-trip a typed command.

### 5.2 Profiles And Imports

- Trigger: user manually adds servers or imports links, JSON, clipboard, QR, or subscriptions.
- Happy path: profiles persist, display, sort, deduplicate, and edit with typed forms.
- Edge cases: malformed links, legacy SIP002 SS links, protocol extras, proxy chain children, filters, multi-URL subscriptions.
- Acceptance notes: all supported protocols round-trip through tests and produce expected generated configs.

### 5.3 Core Config Generation

- Trigger: user connects to a profile or validates a generated config.
- Happy path: context builder resolves profiles, groups, chains, routing, DNS, templates, and core-specific config output.
- Edge cases: ECH SNI extraction, xhttp download settings, policy group observatory ordering, proxy chain dialerProxy or detour, finalmask composition, fakeip, TUN, pre-socks contexts.
- Acceptance notes: golden parity and core acceptance gates pass for Xray and sing-box.

### 5.4 Runtime, Proxy, TUN, And Stats

- Trigger: user connects, disconnects, toggles system proxy, toggles TUN, views logs or speed.
- Happy path: supervisor starts main and pre cores, streams logs, restores proxy or routes, and updates stats.
- Edge cases: sudo password lifecycle, Windows jobs, crash restart, orphan cleanup, PAC Windows-only behavior, concurrent Xray and sing-box stat services.
- Acceptance notes: traffic flows and teardown is clean on every platform.

### 5.5 Advanced Services And Release

- Trigger: user manages routing, DNS, Clash, speedtest, updates, backup, WebDAV, hotkeys, QR, or packaged app installs.
- Happy path: each workflow is wired to real IPC and verified by tests or smoke checks.
- Edge cases: WebDAV XML, release asset OS or arch patterns, regional preset network fetches, signing credentials, notarization, updater metadata.
- Acceptance notes: packaged builds install and launch, and manual release prerequisites are documented.

## 6. Functional Requirements

### 6.1 Data, Config, And IPC

- Port live model fields from `ServiceLib/Models` and omit obsolete columns.
- Preserve enum integer discriminants for `EConfigType` and `ECoreType`.
- Use sqlx SQLite migrations and repositories.
- Generate TypeScript bindings from specta and tauri-specta.
- Expose typed commands and the three event channel model: invalidation, transient streams, and imperative app events.

### 6.2 Profile, Parser, And Subscription Workflows

- Implement CRUD, active server selection, sorting, copying, moving, grouping, deduplication, and `ProfileExItem`.
- Implement per-protocol share format parsers and exporters for vmess, vless, trojan, ss, hysteria2, tuic, wireguard, anytls, naive, socks, and inner `v2rayn://`.
- Implement subscription update with proxy-to-direct fallback, base64 handling, multi-URL, custom UA, conversion target, filters, deduplication, and scheduler.

### 6.3 Config Generation

- Implement `ConfigContextBuilder` equivalent behavior.
- Implement Xray config generation including transports, security, DNS, routing, policy groups, proxy chains, TUN inbound, dokodemo API inbound, stats, templates, and finalmask.
- Implement sing-box config generation including selector or urltest, rule sets, fakeip, typed DNS server schema, clash API, cache file, mux, detour, and templates.
- Implement golden export and canonical diff validation against v2rayN output.

### 6.4 Runtime And Platform Integration

- Implement core launch table for all supported cores.
- Implement process supervisor, dual-process pre-socks handling, sudo or UAC flows, Windows job objects, crash restart, log streaming, and teardown order.
- Implement system proxy, PAC where supported, TUN, autostart, hotkeys, tray, and OS path resolution.

### 6.5 Network Services And UI

- Implement downloads, updates, ruleset and geo acquisition, Clash REST and WebSocket client, WebDAV backup, statistics, speedtest, and UDP testers.
- Implement all expected frontend screens and dialogs with real IPC wiring.
- Implement i18n for 8 languages including RTL, theming, accessibility, virtualization, and performance safeguards.

### 6.6 Packaging

- Implement Tauri packaging configuration for Windows, Linux, and macOS.
- Implement CI workflows for test, binding drift, build, packaging, and release evidence.
- Document manual signing, notarization, credentials, and OS smoke checks outside the automated runner.

## 7. Delivery Strategy

### 7.1 Proposed Phase Shape

- Phase 00: baseline planning evidence and source inventory.
- Phase 01: scaffold, app shell, typed IPC, DB, and CI foundation.
- Phase 02: profiles, server table, parsers, imports, and subscriptions.
- Phase 03: Xray and sing-box config generation plus golden gates.
- Phase 04: runtime alpha with supervisor, connect, system proxy, tray, and statistics.
- Phase 05: routing, DNS, TUN polish, policy groups, proxy chains, and regional presets.
- Phase 06: Clash, speedtest, downloads, updates, geo, and rulesets.
- Phase 07: backup, WebDAV, autostart, hotkeys, QR, i18n, theme, accessibility, and smoke tests.
- Phase 08: packaging, release workflows, runbooks, and final regression evidence.

### 7.2 Rollout And Rollback Notes

- Rollout is batch-based and resumable through `.agents/rollouts/voyavpn-full-rewrite/rollout.py`.
- Each batch must finish with deterministic local verification.
- The v2rayN reference repo remains untouched, so rollback is containment: revert or discard VoyaVPN changes from the target repo.
- Generated IPC, DB migrations, and golden fixtures are treated as contracts once introduced.
- External release steps such as certificates, notarization credentials, and real OS smoke machines remain documented manual checkpoints.

## 8. Technical Boundaries

### 8.1 Likely Repo Areas Touched

- `Cargo.toml`
- `crates/voya-core/**`
- `crates/voya-db/**`
- `crates/voya-platform/**`
- `crates/voya-net/**`
- `crates/voya-udptest/**`
- `crates/voya-app/**`
- `src-tauri/**`
- `src/**`
- `tests/golden/**`
- `docs/**`
- `.github/workflows/**`
- `.agents/rollouts/voyavpn-full-rewrite/**`

### 8.2 Interfaces, Data, Or Contracts

- Rust models derive serde and specta types.
- `src/ipc/bindings.ts` is generated.
- SQLite schema is fresh and forward-only through migrations.
- Core config JSON is validated through canonical golden diffs.
- Events follow the three-channel event model.
- Frontend imports `@tauri-apps/api` only through `src/ipc/**`.

### 8.3 Runtime And Environment Assumptions

- Rust stable toolchain and Cargo.
- Node and pnpm.
- Tauri 2 prerequisites per platform.
- SQLite and sqlx build dependencies.
- Optional core binaries for Xray and sing-box acceptance checks.
- Signing and notarization secrets are not expected in local development.

## 9. External Dependencies And Coordination

- Read-only reference: `/Users/afu/Dev/refs/v2rayN/v2rayN`.
- Detailed planning source: `/Users/afu/.claude/plans/typescript-shadcn-tauri-silly-marble.md`.
- Core binaries and release feeds require network access.
- Signing, notarization, updater key management, and release publishing require manual owner-controlled credentials.
- Cross-platform smoke validation requires Windows, Linux, and macOS environments.

## 10. Hard Rules

- Do not modify `/Users/afu/Dev/refs/v2rayN/v2rayN` or any reference source.
- Do not implement legacy data migration or obsolete columns.
- Do not hand-write TypeScript IPC DTOs that should be generated.
- Do not let UI code import Tauri APIs outside `src/ipc/**`.
- Do not mix OS-specific behavior into `voya-core`.
- Do not assert config generator correctness only through entity snapshots.
- Do not ship GPL or AGPL core binaries in installers by default.
- Keep each batch focused and leave unrelated refactors for later batches.

## 11. Verification And Evidence

### 11.1 Global Verification Commands

- `cargo test --workspace --all-targets`
- `pnpm typecheck`
- `pnpm test -- --run`
- `pnpm lint`
- `pnpm bindings:check`
- `pnpm tauri:build --debug`

### 11.2 Evidence To Capture

- `docs/source-inventory.md`.
- `docs/verification/golden-report.md`.
- `docs/verification/runtime-alpha.md`.
- `docs/verification/cross-platform-smoke.md`.
- `docs/release/runbook.md`.
- CI workflow results.
- Generated prompts and logs under `.agents/rollouts/voyavpn-full-rewrite/logs`.

### 11.3 Batch-Level Verification Guidance

- Prefer batch-local commands first.
- Use workspace-wide checks at phase gates.
- Guard optional external binary checks with clear skip messages when binaries are missing.
- Keep manual OS and signing checks as docs or runbooks, not hidden runner actions.

## 12. Risks, Assumptions, And Open Questions

- Config generation fidelity is the highest technical risk; mitigate with golden exports, canonical diffs, and core acceptance checks.
- TUN and privilege flows are high-risk; isolate in `voya-platform` and document manual OS smoke evidence.
- Large UI surface can drift; mitigate with generated IPC, screen inventory, and feature-complete dialog acceptance.
- Core licensing can affect distribution; default to download-on-first-run and document attribution.
- Toolchain availability is assumed for automated runner execution.
- Open question: final product signing identities, updater keys, and release channels must be supplied outside this rollout.

## 13. Definition Of Done

- All phases and batches in the implementation plan are complete.
- Rust and frontend verification commands pass.
- Generated bindings are drift-free.
- Xray and sing-box golden parity is documented and enforced.
- Runtime alpha workflows pass with a real server.
- System proxy and TUN behavior is smoke-tested on Windows, Linux, and macOS.
- Packaging artifacts or debug installers build for each target OS.
- Manual release and signing prerequisites are documented with owners and rollback notes.
