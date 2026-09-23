import { listen } from "@tauri-apps/api/event";

import { ipcCommands } from "@/ipc/commands";

/**
 * Sole entry point for window chrome.
 *
 * The five controls are thin Rust commands (see `ipc/window.rs`) so the
 * startup path no longer pulls `@tauri-apps/api/window` — and with it
 * `window.js`/`dpi.js`/`image.js`. Resize is the one event that still needs
 * the Tauri event API; `listen("tauri://resize")` is already loaded for the
 * app's own channels.
 *
 * These go through `ipcCommands` (already unwrapped) rather than the raw
 * binding, so they reject with `IpcCommandError` like every other command.
 */

/** Unsubscribe handle returned by the window event listeners below. */
type WindowUnlisten = () => void;

/** Whether the page runs inside the Tauri shell, rather than a plain browser or a test. */
export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function minimizeWindow(): Promise<void> {
  await ipcCommands.minimizeWindow();
}

export async function toggleMaximizeWindow(): Promise<void> {
  await ipcCommands.toggleMaximizeWindow();
}

export async function closeWindow(): Promise<void> {
  await ipcCommands.closeWindow();
}

export function isWindowMaximized(): Promise<boolean> {
  return ipcCommands.isWindowMaximized();
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
