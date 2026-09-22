# @voya/contracts

Generated TypeScript view of `crates/voya-contracts`. All four artifacts are
derived from `apps/desktop/src/ipc/bindings.ts` by
`scripts/quality/contracts-source.mjs` — never edit them, and never hand-write
types that mirror backend types anywhere else.

- `src/generated.ts` (exported as `@voya/contracts`) — every IPC DTO,
  `VoyaCommands` with the result envelope unwrapped, and `VoyaEventChannels`
  for the three event channels. Consumed by every frontend.
- `src/commands.ts` (exported as `@voya/contracts/commands`) —
  `VOYA_COMMAND_WIRE`: each command's wire name and argument names, so a
  non-Tauri transport can rebuild Tauri's named-argument object from a
  positional call.
- `commands.json` — every command wire name alone. `crates/voya-mobile-ffi`
  reads it in its Rust coverage tests: each name must have a dispatcher or an
  explicit mobile decision.
- `events.json` — each event channel's wire name and the `kind` values its
  payload can carry, checked against the mobile host's own event enums.

Run `pnpm generate:bindings` after changing any Rust command, event or DTO;
`pnpm run check:bindings` fails on drift.
