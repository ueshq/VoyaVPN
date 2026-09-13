import { beforeEach, describe, expect, it, vi } from "vitest";

const plugin = vi.hoisted(() => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));
const windowApi = vi.hoisted(() => ({ isWindowVisible: vi.fn() }));

vi.mock("@tauri-apps/plugin-notification", () => plugin);
vi.mock("@/ipc/window", () => windowApi);

// A fresh module is a fresh launch: the "asked once" memory is module state.
async function launch() {
  vi.resetModules();
  return import("./notifications");
}

describe("notifyWhenHidden", () => {
  beforeEach(() => {
    Object.values(plugin).forEach((mock) => mock.mockReset());
    windowApi.isWindowVisible.mockReset();
  });

  it("stays quiet while the window is on screen", async () => {
    windowApi.isWindowVisible.mockResolvedValue(true);
    const { notifyWhenHidden } = await launch();

    await expect(notifyWhenHidden("Connection lost")).resolves.toBe(false);
    expect(plugin.isPermissionGranted).not.toHaveBeenCalled();
    expect(plugin.sendNotification).not.toHaveBeenCalled();
  });

  it("notifies while the window is hidden and permission is granted", async () => {
    windowApi.isWindowVisible.mockResolvedValue(false);
    plugin.isPermissionGranted.mockResolvedValue(true);
    const { notifyWhenHidden } = await launch();

    await expect(notifyWhenHidden("Connection lost")).resolves.toBe(true);
    expect(plugin.sendNotification).toHaveBeenCalledExactlyOnceWith({ title: "Connection lost" });
    expect(plugin.requestPermission).not.toHaveBeenCalled();
  });

  it("asks for permission once per launch and respects a refusal", async () => {
    windowApi.isWindowVisible.mockResolvedValue(false);
    plugin.isPermissionGranted.mockResolvedValue(false);
    plugin.requestPermission.mockResolvedValue("denied");
    const { notifyWhenHidden } = await launch();

    await expect(notifyWhenHidden("First")).resolves.toBe(false);
    await expect(notifyWhenHidden("Second")).resolves.toBe(false);
    expect(plugin.requestPermission).toHaveBeenCalledOnce();
    expect(plugin.sendNotification).not.toHaveBeenCalled();

    plugin.requestPermission.mockResolvedValue("granted");
    const next = await launch();
    await expect(next.notifyWhenHidden("After relaunch")).resolves.toBe(true);
    expect(plugin.sendNotification).toHaveBeenCalledExactlyOnceWith({ title: "After relaunch" });
  });

  it("never rejects when the plugin fails", async () => {
    windowApi.isWindowVisible.mockRejectedValue(new Error("no window"));
    const { notifyWhenHidden } = await launch();

    await expect(notifyWhenHidden("Connection lost")).resolves.toBe(false);
  });
});
