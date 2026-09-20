import { describe, expect, it, vi } from "vitest";

import { appVisibilityAdapter, clipboard, requestElevation } from "@voya/client/platform";
import { voyaCommands } from "@voya/client/transport";

const ipcMocks = vi.hoisted(() => ({
  readClipboardText: vi.fn(),
  tunRequestElevation: vi.fn(),
}));

vi.mock("@/ipc/commands", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/ipc/commands")>()),
  ...ipcMocks,
}));

const writeText = vi.hoisted(() => vi.fn());
vi.mock("@/lib/clipboard", () => ({ writeClipboard: writeText }));

// The registration itself runs in `src/test/setup.ts`, per test, exactly as
// `platform-boot` runs it once at startup.
describe("registerDesktopBackend", () => {
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
