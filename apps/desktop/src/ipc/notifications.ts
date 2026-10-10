import { ipcCommands } from "@/ipc/commands";

/**
 * Sole entry point for OS notifications.
 *
 * Toasts cover everything that happens while the window is on screen. Once it
 * is hidden in the tray they reach nobody, so the few notices a user must not
 * miss are repeated as an OS notification — and only then, so a user looking
 * at the app never gets the same message twice.
 *
 * The shell posts it, and with it owns the permission: macOS asks the user
 * the first time, Windows and Linux have nothing to ask.
 */

/** Asks for an OS notification while the main window is hidden. Resolves whether it asked. */
export async function notifyWhenHidden(title: string): Promise<boolean> {
  try {
    // On screen rather than hidden into the tray: the toast is enough.
    if (await ipcCommands.isWindowVisible()) return false;
    await ipcCommands.showNotification(title);
    return true;
  } catch {
    // The toast already carries the notice, so a failure here must never
    // become an unhandled rejection.
    return false;
  }
}
