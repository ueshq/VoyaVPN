import { listen } from "@tauri-apps/api/event";

import { ipcCommands } from "@/ipc/commands";

/**
 * Sole entry point for window chrome.
 *
 * The window controls are thin Rust commands (see `ipc/window.rs`) reached
 * through `voyaCommands()`, so the startup path never pulls
 * `@tauri-apps/api/window` — and with it `window.js`/`dpi.js`/`image.js`.
 * Resize is the one event that still needs the Tauri event API;
 * `listen("tauri://resize")` is already loaded for the app's own channels.
 */

/** Unsubscribe handle returned by the window event listeners below. */
type WindowUnlisten = () => void;

/** Whether the page runs inside the Tauri shell, rather than a plain browser or a test. */
export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Whether the window is on screen rather than hidden into the tray. */
export function isWindowVisible(): Promise<boolean> {
  return ipcCommands.isWindowVisible();
}

/** Watch for size changes so the title bar can swap the maximize/restore icon. */
export async function onWindowResized(handler: () => void): Promise<WindowUnlisten> {
  return listen("tauri://resize", () => {
    handler();
  });
}
