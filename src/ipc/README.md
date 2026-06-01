# IPC

Only this directory may import `@tauri-apps/api`.

`bindings.ts` is generated from Rust `specta` and `tauri-specta` definitions.
Do not edit it by hand and do not add hand-written TypeScript DTOs for backend
commands or events.

Use `commands.ts` for typed command wrappers and `event-bridge.tsx` for the
single mounted event bridge.
