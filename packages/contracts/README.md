# @voya/contracts

Generated TypeScript view of `crates/voya-contracts`: every IPC DTO, the
`VoyaCommands` surface with the result envelope unwrapped, and the three event
channels.

`src/generated.ts` is derived from `apps/desktop/src/ipc/bindings.ts` by
`scripts/quality/contracts-source.mjs`. Run `pnpm generate:bindings` after
changing any Rust command, event or DTO; `pnpm check:bindings` fails on drift.

Do not edit `src/generated.ts`, and do not hand-write types that mirror backend
types anywhere else.
