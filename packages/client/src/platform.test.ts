import { describe, expect, it, vi } from "vitest";

import {
  appVisibilityAdapter,
  clientStorage,
  clipboard,
  requestElevation,
  setAppVisibility,
  setClientStorage,
  setClipboard,
  setElevationHandler,
} from "./platform";

describe("clientStorage", () => {
  it("forgets rather than throwing before a platform registers", () => {
    // Stores are created at module scope, so reading one before startup wiring
    // has to be survivable.
    expect(clientStorage().getItem("voyavpn.preferences")).toBeNull();
  });

  it("forwards to the storage registered after the handle was taken", () => {
    // zustand's createJSONStorage resolves the getter once, while the store
    // module is evaluated — so the handle has to stay valid across a later
    // registration or a store would be stuck on the in-memory fallback.
    const handle = clientStorage();
    const entries = new Map<string, string>();

    setClientStorage({
      getItem: (name) => entries.get(name) ?? null,
      setItem: (name, value) => entries.set(name, value),
      removeItem: (name) => entries.delete(name),
    });

    handle.setItem("k", "v");
    expect(handle.getItem("k")).toBe("v");
    expect(entries.get("k")).toBe("v");

    handle.removeItem("k");
    expect(handle.getItem("k")).toBeNull();
  });
});

describe("requestElevation", () => {
  it("declines for a platform that registered no prompt", async () => {
    // No handler means no retry, rather than a call into a command this
    // platform may not even have.
    await expect(requestElevation()).resolves.toBe(false);
  });

  it("asks the registered handler, every time", async () => {
    const handler = vi.fn().mockResolvedValueOnce(true).mockResolvedValueOnce(false);
    setElevationHandler(handler);

    await expect(requestElevation()).resolves.toBe(true);
    await expect(requestElevation()).resolves.toBe(false);
    expect(handler).toHaveBeenCalledTimes(2);
  });
});

describe("clipboard", () => {
  it("names the missing registration instead of failing with a TypeError", () => {
    expect(() => clipboard().readText()).toThrow(/No clipboard registered/);
    expect(() => clipboard().writeText("vless://node")).toThrow(/No clipboard registered/);
  });

  it("forwards both halves to the registered adapter", async () => {
    const readText = vi.fn().mockResolvedValue("vless://node");
    const writeText = vi.fn().mockResolvedValue(undefined);
    setClipboard({ readText, writeText });

    await expect(clipboard().readText()).resolves.toBe("vless://node");
    await clipboard().writeText("vless://other");
    expect(writeText).toHaveBeenCalledWith("vless://other");
  });
});

describe("appVisibilityAdapter", () => {
  it("reports the app as on screen until a platform says otherwise", () => {
    // A missing registration must not silently switch every live stream off.
    const adapter = appVisibilityAdapter();
    expect(adapter.isVisible()).toBe(true);
    expect(adapter.subscribe(() => {})()).toBeUndefined();
  });

  it("reports what the registered adapter says and unsubscribes through it", () => {
    const unsubscribe = vi.fn();
    const subscribe = vi.fn().mockReturnValue(unsubscribe);
    setAppVisibility({ isVisible: () => false, subscribe });

    const adapter = appVisibilityAdapter();
    expect(adapter.isVisible()).toBe(false);
    adapter.subscribe(() => {})();
    expect(unsubscribe).toHaveBeenCalledOnce();
  });
});
