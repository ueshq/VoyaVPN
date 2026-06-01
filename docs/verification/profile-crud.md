# Profile CRUD Verification

Batch: `02-01-profile-crud-managers`

## Scope

- Added persisted profile CRUD through `voya-db::ProfileRepository`.
- Added `ProfileExRepository` for sort, delay, speed, message, and IP info state.
- Added `voya_app::profiles::ProfileManager` for create, edit, delete, copy, move, sort, group move, dedupe, and active-profile fallback.
- Added typed Tauri commands for profile operations and emitted `InvalidateEvent` keys for profile, ProfileEx, and active-profile changes.
- Regenerated `src/ipc/bindings.ts` from Rust types.

## Reference Behavior

- `ConfigHandler.AddServerCommon` was used for profile ID, config version, stream-security defaulting, network defaulting, and sort behavior.
- `ConfigHandler.MoveServer`, `SortServers`, `DedupServerList`, `MoveToGroup`, and `SetDefaultServerIndex` shaped the manager operations.
- `ProfileExManager` shaped sort and speed-test metadata persistence.

## Verification Commands

Run on 2026-05-31:

```sh
cargo test -p voya-app profile --all-targets
cargo test -p voya-db profile --all-targets
pnpm bindings:check
```

Result: all three commands passed locally.

## Notes

- No external core binaries or network services are required for this batch.
- Frontend server-table UI is intentionally left for `02-02-server-table-dialogs`; this batch exposes the typed IPC surface it will consume.
