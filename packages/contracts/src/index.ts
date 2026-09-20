/**
 * The backend contract, shared by every frontend.
 *
 * `generated.ts` is derived from `apps/desktop/src/ipc/bindings.ts` (itself
 * generated from the Rust `specta` types) by
 * `scripts/quality/contracts-source.mjs`, and `pnpm check:bindings` fails when
 * the two drift. Nothing here is hand-written, and nothing here knows about a
 * transport: the desktop shell reaches these commands over Tauri IPC, a mobile
 * app reaches them over a native module.
 */
export * from "./generated";
