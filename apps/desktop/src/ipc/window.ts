import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * Sole entry point for the Tauri window plugin API.
 *
 * Window controls talk to the window plugin directly (they do not go through a
 * Rust IPC command), so the custom title bar drives the current window only via
 * this module. No other file may import `@tauri-apps/api/window`.
 */

/** Unsubscribe handle returned by the window event listeners below. */
type WindowUnlisten = () => void;

/** Whether the page runs inside the Tauri shell, rather than a plain browser or a test. */
export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function minimizeWindow(): Promise<void> {
  return getCurrentWindow().minimize();
}

export function toggleMaximizeWindow(): Promise<void> {
  return getCurrentWindow().toggleMaximize();
}

export function closeWindow(): Promise<void> {
  return getCurrentWindow().close();
}

export function isWindowMaximized(): Promise<boolean> {
  return getCurrentWindow().isMaximized();
}

/** Whether the window is on screen rather than hidden into the tray. */
export function isWindowVisible(): Promise<boolean> {
  return getCurrentWindow().isVisible();
}

/** Watch for size changes so the title bar can swap the maximize/restore icon. */
export function onWindowResized(handler: () => void): Promise<WindowUnlisten> {
  return getCurrentWindow().onResized(handler);
}
