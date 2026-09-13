import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

import { isWindowVisible } from "@/ipc/window";

/**
 * Sole entry point for the Tauri notification plugin.
 *
 * Toasts cover everything that happens while the window is on screen. Once it
 * is hidden in the tray they reach nobody, so the few notices a user must not
 * miss are repeated as an OS notification — and only then, so a user looking
 * at the app never gets the same message twice.
 */

// Asked at most once per launch: a user who declined is not asked again on
// every dropped connection.
let permissionRequested = false;

async function permissionGranted() {
  if (await isPermissionGranted()) return true;
  if (permissionRequested) return false;
  permissionRequested = true;
  return (await requestPermission()) === "granted";
}

/** Shows an OS notification while the main window is hidden. Resolves whether one was shown. */
export async function notifyWhenHidden(title: string): Promise<boolean> {
  try {
    if (await isWindowVisible()) return false;
    if (!(await permissionGranted())) return false;
    sendNotification({ title });
    return true;
  } catch {
    // No notification service, or a revoked permission: the toast already
    // carries the notice, so this must never become an unhandled rejection.
    return false;
  }
}
