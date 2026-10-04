import { listen } from "@tauri-apps/api/event";

/**
 * Sole entry point for window chrome.
 *
 * The window controls are thin Rust commands (see `ipc/window.rs`) that
 * features reach through `voyaCommands()` like any other, so the startup path
 * never pulls `@tauri-apps/api/window` — and with it
 * `window.js`/`dpi.js`/`image.js`. What is left here is the runtime check and
 * resize, the one event that still needs the Tauri event API;
 * `listen("tauri://resize")` is already loaded for the app's own channels.
 */

/** Unsubscribe handle returned by the window event listeners below. */
type WindowUnlisten = () => void;

/** Whether the page runs inside the Tauri shell, rather than a plain browser or a test. */
export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Watch for size changes so the title bar can swap the maximize/restore icon. */
export async function onWindowResized(handler: () => void): Promise<WindowUnlisten> {
  return listen("tauri://resize", () => {
    handler();
  });
}
