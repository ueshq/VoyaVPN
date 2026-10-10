import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

// The two commands this module calls. The shared backend registration the
// test setup runs takes the same object, and only passes it along.
const shell = vi.hoisted(() => ({
  isWindowVisible: vi.fn(),
  showNotification: vi.fn(),
}));

vi.mock("@/ipc/commands", () => ({ ipcCommands: shell }));

import { notifyWhenHidden } from "./notifications";

describe("notifyWhenHidden", () => {
  beforeEach(() => {
    Object.values(shell).forEach((mock) => mock.mockReset());
  });

  it("stays quiet while the window is on screen", async () => {
    shell.isWindowVisible.mockResolvedValue(true);

    await expect(notifyWhenHidden("Connection lost")).resolves.toBe(false);
    expect(shell.showNotification).not.toHaveBeenCalled();
  });

  it("asks the shell for a notification while the window is hidden", async () => {
    shell.isWindowVisible.mockResolvedValue(false);
    shell.showNotification.mockResolvedValue(undefined);

    await expect(notifyWhenHidden("Connection lost")).resolves.toBe(true);
    expect(shell.showNotification).toHaveBeenCalledExactlyOnceWith("Connection lost");
  });

  it("never rejects when the shell fails", async () => {
    shell.isWindowVisible.mockResolvedValue(false);
    shell.showNotification.mockRejectedValue(new Error("no notification service"));
    await expect(notifyWhenHidden("Connection lost")).resolves.toBe(false);

    shell.isWindowVisible.mockRejectedValue(new Error("no window"));
    await expect(notifyWhenHidden("Connection lost")).resolves.toBe(false);
  });
});
