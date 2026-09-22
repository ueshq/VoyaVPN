---
feature: reduce-complexity
status: in-progress
updated: 2026-09-22
branch: refactor/reduce-complexity
commits:
---

# Reduce Complexity

## Report

## [S1] Problem

The monorepo still carries more indirection, compatibility residue, and duplicated
implementation than the product needs. Two earlier passes (`fa9b11b` frontend,
`c348f40` Rust) and the gluestack removal (`36a7268`) deleted the loudest cases;
what remains is quieter but still taxes every change:

- **Host↔host copy-paste.** Desktop Tauri commands and `voya-mobile-ffi` dispatch
  reimplement the same pure maps, post-commit tails, monitor reporting, and
  manager construction. A new enum variant or field must be edited twice or the
  hosts drift.
- **Twin shell modules.** `query-client`, `useRuntimeStatusSeed`, and node-list
  selection state exist once under `apps/desktop` and once under `apps/mobile`
  with only platform deltas. Desktop also bypasses the `@voya/client` visibility
  seam it registers.
- **Pass-through and name-compat layers.** One-line wrappers (`mutate_config`,
  `current_config`, manager `list_*`, `QrCodeManager` namespace, `ExportManager`),
  a historical `QrNotFoundError` name shim, pure `voya_contracts` re-exports, and
  five `validate_*_ipc_*` error remaps add hops without policy.
- **Dead and stale surface.** `AppMetadata`, `NoopStatisticsEventSink`, mobile
  `zero_statistics`, an error re-export facade, renamed-module comments, and a
  generated comment that still points at `apps/desktop/src/ipc/query-keys.ts`.
- **Empty suppression machinery.** Three allowlist systems currently suppress
  nothing and freeze that fact with emptiness tests.

Goal: cut complexity tax with maintainability first — delete what is dead, merge
what is duplicated, extract what both hosts need — without redesigning the
IPC contract or the domain/contract split.

## [S2] Design

### Principles

1. **Delete before abstract.** Dead code and name-compat shims go first; no
   replacement API is invented for them.
2. **One owner per policy.** If both hosts need the same sequence or map, it
   lives in `voya-app` (usually `contract_map` / `post_commit` / `proxy_runtime`),
   not in either shell.
3. **Keep architecture gates intact.** Shell stays off `voya_core`/`voya_db`;
   `voya-app` stays off `reqwest`/`tokio::process`/`specta`; `voya-core` stays
   OS-free; `voya-contracts` stays the only `specta::Type` crate. Facades that
   exist *for* those gates (`config_mutation::{AppConfig, UnitOfWork}`,
   `startup::{DATABASE_NAME, DatabaseBackup, manual_database_reset_command}`,
   event-sink traits with two production impls) are kept.
4. **No behavior change.** Toast visibility, IPC wire shapes, generated
   bindings, and sing-box output stay byte-stable except where a rename forces
   test updates.

### Host map and post-commit extraction

Pure domain→contract maps and the shared post-commit tail move into `voya-app`:

| Symbol | New home | Replaces |
| --- | --- | --- |
| `core_flow_log_level` | `voya-app::contract_map` | hand maps in both `dispatch/runtime.rs` and `ipc/commands/runtime.rs` |
| `core_flow_notice_level` | same | same |
| `core_state_to_contract` | same | same |
| `connection_ip_to_contract` | same | hand maps in `ipc/commands/speedtest.rs` and `dispatch/runtime.rs` |
| `process_log_level_to_contract` | same (moved from `logging.rs`) | out-of-family map |
| `report_monitor_result` | `voya-app::proxy_runtime` | `report_monitor*` in both hosts |
| `finish_config_change` | `voya-app::post_commit` | `finish_config_change` (mobile) + `restart_after_config_change`/`finish_routing_change` (desktop) |

`zero_statistics` is deleted; mobile calls
`statistics_snapshot_to_contract(StatisticsSnapshot::zero())` as desktop already
does.

Manager construction policy (`tun_manager`, `system_proxy_manager`,
`runtime_with_seed`) moves onto `AppServices` so both hosts share one recipe.
Mobile holds one `SystemProxyManager` instead of allocating per call.

### Frontend shell merge

| Twin | Shared home | Host keeps |
| --- | --- | --- |
| QueryClient factory | `@voya/client` `createAppQueryClient(opts)` | the single option delta |
| `useRuntimeStatusSeed` | `@voya/features` (visibility via `appVisibilityAdapter`) | nothing; desktop drops raw `document`/`focus` |
| Node-list selection state | `@voya/features` (optional `searchRef`) | a thin desktop adapter for the DOM ref |

`QrNotFoundError` name-compat is removed in favour of `QrScanError`; only tests
asserted the historical name.

`apps/desktop/src/ipc/commands.ts` stops re-exporting `@voya/client/errors`;
callers import the errors module directly. The desktop icon map file is renamed
`import-method-icons.ts` so it no longer shadows the shared `import-methods.ts`.

`modal-store.ts` folds into `runtime-action-store` (single `missingCore` modal).

### Pass-through and dead-code deletion

- Delete `AppMetadata`/`metadata()`, `NoopStatisticsEventSink`.
- Replace `clean_optional_string` with `voya_core::text` helpers.
- Inline shell `mutate_config` / `current_config` one-liners at call sites
  (mobile already calls `config_mutations` directly).
- Collapse five `validate_*_ipc_*` wrappers into one `map_ipc_input` helper.
- Inline manager `list_*` repository pass-throughs and `ExportManager` /
  `QrCodeManager` unit-struct namespaces into free functions or direct repo
  reads through `AppServices`.
- `ConnectionModeManager::commit` stops reimplementing
  `ConfigMutationCoordinator::mutate` and reports `config_changed`.
- Delete pure `pub use voya_contracts::…` re-exports; keep the `voya_db` /
  `voya_core` facades listed under Principles.
- Delete the three empty allowlist machines (`KNOWN_UNSAFE_WITHOUT_SAFETY_COMMENT`,
  `KNOWN_HARDCODED_TEXT`, `untestedModules`) and their emptiness tests.

### Documentation hygiene

- Point `invalidation.rs` and `voya-mobile-ffi/src/app.rs` comments at
  `post_commit.rs` / `voya-app/src/lifecycle.rs` (names after `c348f40`).
- Point the generated `InvalidationScope` comment at `packages/client/src/query-keys.ts`.
- List all four `packages/contracts` artifacts in its README.

### Deferred (specified, not in this delivery)

These are real complexity taxes but need focused, separately reviewed changes:

1. Collapse `contract_map` pure-rename pairs so domain and contract share one
   type tree (or generate one side).
2. Drop the desktop `CommandResult`/`wrapCommand` envelope and make
   `bindings.ts` re-export types from `@voya/contracts` (single type identity).
3. Generate event-channel wire names into mobile TS / Rust / Swift / Kotlin
   from `events.json`.
4. Merge `RuntimeManager` into `CoreFlow` (and the speedtest trait stack).
5. Replace single-impl test-seam traits with generics/closures (self_host cluster
   first).
6. Give the mobile toast pipeline a platform sink (behavior, not deletion).
7. Unify storage (`shell-store`, i18n locale) onto `ClientStorage`.
8. Shrink `TECHNICAL_TEXT_ALLOWLIST` to true wire identifiers with stale-entry
   reporting.

## [S3] Out of Scope

- Any change to sing-box generated JSON, IPC command/event wire names, or
  `check:bindings` generated files except comment/path fixes inside them.
- v2rayN compatibility reintroduction (already forbidden by the architecture gate).
- Mobile toast UX product design, Android/iOS native UI work.
- `apps/web` (placeholder with no source).
- Performance work, dependency upgrades, or CI topology changes.
- The deferred list in [S2] — documented there so it is not lost, not implemented
  here.

## Tasks

- [ ] T1: Extract host-shared pure maps into `voya-app::contract_map` (`core_flow_log_level`, `core_flow_notice_level`, `core_state_to_contract`, `connection_ip_to_contract`, move `process_log_level_to_contract`) — acceptance: both hosts compile against the shared fns; the hand-rolled copies are gone; `pnpm run check:architecture` and `pnpm run check:rust:clippy` pass (covers: S2)
- [ ] T2: Share `report_monitor_result` and `finish_config_change`; delete mobile `zero_statistics` — acceptance: one implementation each in `voya-app`; hosts call them; mobile statistics zero path uses `StatisticsSnapshot::zero()` (covers: S2; depends: T1)
- [ ] T3: Move manager construction onto `AppServices` and stop per-call `SystemProxyManager` allocation on mobile — acceptance: both hosts obtain tun/system-proxy/runtime through `AppServices`; mobile holds one `SystemProxyManager` (covers: S2; depends: T2)
- [ ] T4: Delete dead and name-compat surface (`QrNotFoundError` shim → `QrScanError`, `AppMetadata`, `NoopStatisticsEventSink`, `commands.ts` error re-export, pure `voya_contracts` re-exports) — acceptance: `pnpm run check:dead-code` and affected tests pass; no production caller references the old names (covers: S2)
- [ ] T5: Inline pass-throughs (`clean_optional_string`, shell `mutate_config`/`current_config`, `validate_*_ipc_*` → `map_ipc_input`, manager `list_*`, `ExportManager`, `QrCodeManager`, `ConnectionModeManager::commit` → `mutate`) — acceptance: call sites use the underlying API or the single shared helper; `pnpm run check:rust:test` passes (covers: S2)
- [ ] T6: Merge frontend twins (`createAppQueryClient`, shared `useRuntimeStatusSeed` via `appVisibilityAdapter`, shared node-list selection) — acceptance: one factory/hook each under `@voya/client`/`@voya/features`; desktop uses the visibility seam; mobile/desktop tests pass (covers: S2)
- [ ] T7: Fold `modal-store` into `runtime-action-store`; rename desktop `import-methods.ts` → `import-method-icons.ts` — acceptance: single modal state owner; icon map filename no longer collides with the shared module (covers: S2)
- [ ] T8: Documentation hygiene (stale `lifecycle.rs`/`query-keys` paths, `packages/contracts` README lists four artifacts) — acceptance: no comment names a deleted module; README names `generated.ts`, `commands.ts`, `commands.json`, `events.json` (covers: S2)
- [ ] T9: Delete empty allowlist machinery (`KNOWN_UNSAFE_WITHOUT_SAFETY_COMMENT`, `KNOWN_HARDCODED_TEXT`, `untestedModules`) and their emptiness tests — acceptance: the three suppressions and their emptiness tests are gone; `pnpm run check:architecture` and `pnpm run check:i18n` still pass (covers: S2)
