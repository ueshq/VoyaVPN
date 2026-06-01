# Server Table And Profile Dialogs Verification

Batch: `02-02-server-table-dialogs`

## Scope

- Profiles tab renders a TanStack Table row model through TanStack Virtual.
- Row actions use the typed profile IPC wrappers from `src/ipc`: list, save, delete, copy, move, sort, dedupe, and activate.
- Add/edit dialog uses `react-hook-form` with a zod discriminated union on `ConfigType`.
- Form paths exist for VMess, Custom, Shadowsocks, SOCKS, VLESS, Trojan, Hysteria2, TUIC, WireGuard, HTTP, AnyTLS, Naive, Policy Group, and Proxy Chain.

## Frontend Evidence

- `src/features/profiles/server-table.test.tsx` covers:
  - 5,000 profile rows with virtualized DOM row count.
  - Activate, copy, delete, sort, and drag-position move through IPC wrapper mocks.
  - Add-profile dialog protocol options and a WireGuard submit path through `saveProfile`.

## Verification Commands

- `pnpm typecheck` - passed.
- `pnpm test -- --run` - passed.
- `pnpm lint` - passed.

## Deferred External Checks

- No real Tauri runtime was launched in this batch. The batch acceptance is frontend IPC wiring and behavior; runtime persistence is covered by the previous profile CRUD manager batch and later end-to-end smoke checks.
