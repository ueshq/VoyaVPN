# Subscription Verification

Batch: `02-04-import-subscriptions`

## Scope

- Added `voya-net` HTTP download and subscription fetch helpers with proxy-to-direct fallback, custom user-agent headers, base64 payload decoding, `MoreUrl` merging, and `ConvertTarget` URL rewriting.
- Added `voya-db::SubscriptionRepository` for the fresh `subscriptions` table.
- Added `voya-app::subscriptions::SubscriptionManager` for save/delete/import/update flows. Regex filtering and profile dedupe happen in this manager layer after parsing and before persistence.
- Added a deterministic `SubscriptionScheduler` test hook that starts from an injected tick channel and stops cleanly.
- Added Tauri IPC commands and generated TypeScript bindings for subscriptions, text/file imports, updates, and due-update runs.
- Added Profiles-screen dialogs for manual paste/file imports and subscription management/update actions.

## Reference Notes

- v2rayN reference paths read:
  - `ServiceLib/Handler/SubscriptionHandler.cs`
  - `ServiceLib/Handler/ConfigHandler.cs`
  - `ServiceLib/Manager/TaskManager.cs`
  - `ServiceLib/Models/Entities/SubItem.cs`
  - `ServiceLib/ViewModels/SubSettingViewModel.cs`
  - `ServiceLib/ViewModels/SubEditViewModel.cs`
- Matching behavior ported in this batch:
  - Main subscription fetch uses conversion URL when `ConvertTarget` is set.
  - `MoreUrl` entries are fetched and appended only when conversion is not active.
  - Proxy fetch falls back to direct fetch when proxy download fails or is empty.
  - Per-subscription `UserAgent` is sent on main and additional downloads.
  - Subscription imports remove prior subscription-owned profiles, apply regex filter, dedupe matching profiles, persist new profiles, and preserve active selection when a replacement matches.

## Local Evidence

Captured on 2026-05-31:

- PASS: `cargo test -p voya-net --all-targets`
- PASS: `cargo test -p voya-app subscription --all-targets`
- PASS: `pnpm typecheck`
- PASS: `pnpm test -- --run`

## External Checks

No real third-party subscription URL was used in this batch. The network behavior is covered with local fixture HTTP servers so tests are deterministic and do not leak private subscription URLs.
