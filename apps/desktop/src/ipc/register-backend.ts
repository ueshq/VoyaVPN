import {
  setAppVisibility,
  setClipboard,
  setElevationHandler,
} from "@voya/client/platform";
import { setVoyaCommands } from "@voya/client/transport";

import { ipcCommands } from "@/ipc/commands";
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
  setVoyaCommands(ipcCommands);

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
}
