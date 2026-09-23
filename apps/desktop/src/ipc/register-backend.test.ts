import { beforeEach, describe, expect, it, vi } from "vitest";

import { appVisibilityAdapter, clipboard, requestElevation } from "@voya/client/platform";
import { voyaCommands } from "@voya/client/transport";

import { registerDesktopBackend } from "./register-backend";

const ipcMocks = vi.hoisted(() => ({
  readClipboardText: vi.fn(),
  tunRequestElevation: vi.fn(),
}));

// `registerDesktopBackend` puts `ipcCommands` on the shared transport, so the
// unit under test is that wiring: replace the mapped object itself and then run
// the same registration `platform-boot` performs at startup.
vi.mock("@/ipc/commands", () => ({ ipcCommands: ipcMocks }));

const writeText = vi.hoisted(() => vi.fn());
vi.mock("@/lib/clipboard", () => ({ writeClipboard: writeText }));

describe("registerDesktopBackend", () => {
  beforeEach(() => {
    ipcMocks.readClipboardText.mockReset();
    ipcMocks.tunRequestElevation.mockReset();
    writeText.mockReset();
    registerDesktopBackend();
  });

  it("puts the whole Tauri command surface behind the shared transport", () => {
    expect(voyaCommands().readClipboardText).toBe(ipcMocks.readClipboardText);
  });

  it("reads the clipboard through the backend and writes through the WebView", async () => {
    ipcMocks.readClipboardText.mockResolvedValueOnce("vless://node");

    // A WebView read asks the user to confirm "Paste" every time, so a paste
    // menu item has to go the long way round.
    await expect(clipboard().readText()).resolves.toBe("vless://node");
    await clipboard().writeText("vless://other");
    expect(writeText).toHaveBeenCalledWith("vless://other");
  });

  it("answers the elevation prompt with what the backend dialog returned", async () => {
    ipcMocks.tunRequestElevation.mockResolvedValueOnce({ elevationGranted: true });
    await expect(requestElevation()).resolves.toBe(true);

    ipcMocks.tunRequestElevation.mockResolvedValueOnce({ elevationGranted: false });
    await expect(requestElevation()).resolves.toBe(false);
  });

  it("reports the window as visible while the document is", () => {
    expect(appVisibilityAdapter().isVisible()).toBe(true);
  });
});
