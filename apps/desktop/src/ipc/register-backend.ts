import {
  setAppVisibility,
  setBackendAvailable,
  setClipboard,
  setElevationHandler,
} from "@voya/client/platform";
import { setVoyaCommands } from "@voya/client/transport";

import { ipcCommands } from "@/ipc/commands";
import { isTauriRuntime } from "@/ipc/window";
import { writeClipboard } from "@/lib/clipboard";
import { isDocumentVisible, subscribeToDocumentVisible } from "@/lib/document-visible";

/**
 * Puts the desktop behind the seams `@voya/client` reaches the backend through.
 *
 * A function rather than module side effects so the unit tests can run it
 * after their own `installFakeCommands` is in place; `platform-boot`
 * calls it once at startup.
 */
export function registerDesktopBackend() {
  // `satisfies` is the whole point of the seam: dropping or mistyping a command
  // in the Tauri binding fails the build here rather than at the call site.
  setVoyaCommands(ipcCommands satisfies VoyaCommandsShape);

  // TUN needs one system authorization on Unix; the backend shows the native
  // dialog and answers whether it was granted.
  setElevationHandler(async () => (await ipcCommands.tunRequestElevation()).elevationGranted);

  // Reads go through the backend because a WebView read needs a user gesture
  // the paste menu item does not always carry; writes go through the WebView.
  setClipboard({
    readText: () => ipcCommands.readClipboardText(),
    writeText: writeClipboard,
  });

  setAppVisibility({ isVisible: isDocumentVisible, subscribe: subscribeToDocumentVisible });

  // The frontend-only dev server (`pnpm dev:web`) has no backend behind it, so
  // anything that would start a stream the backend must later stop asks first.
  setBackendAvailable(isTauriRuntime);
}

// Declared locally so the assertion reads as one line above; `VoyaCommands`
// itself is the generated contract.
type VoyaCommandsShape = import("@voya/contracts").VoyaCommands;
