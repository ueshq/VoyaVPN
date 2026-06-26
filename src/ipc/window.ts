import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * Sole entry point for the Tauri window plugin API.
 *
 * Window controls talk to the window plugin directly (they do not go through a
 * Rust IPC command), so the custom title bar drives the current window only via
 * this module. No other file may import `@tauri-apps/api/window`.
 */

/** Unsubscribe handle returned by the window event listeners below. */
export type WindowUnlisten = () => void;

export function minimizeWindow(): Promise<void> {
  return getCurrentWindow().minimize();
}

export function toggleMaximizeWindow(): Promise<void> {
  return getCurrentWindow().toggleMaximize();
}

export function closeWindow(): Promise<void> {
  return getCurrentWindow().close();
}

/** Begin an interactive window drag (called from the self-drawn drag region). */
export function startWindowDragging(): Promise<void> {
  return getCurrentWindow().startDragging();
}

export function isWindowMaximized(): Promise<boolean> {
  return getCurrentWindow().isMaximized();
}

/** Watch for size changes so the title bar can swap the maximize/restore icon. */
export function onWindowResized(handler: () => void): Promise<WindowUnlisten> {
  return getCurrentWindow().onResized(handler);
}
