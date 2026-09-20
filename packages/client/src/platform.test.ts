import { describe, expect, it } from "vitest";

import { clientStorage, setClientStorage, setSystemColorSchemeReader, systemColorScheme } from "./platform";

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

describe("systemColorScheme", () => {
  it("defaults to light for a host that reports no preference", () => {
    expect(systemColorScheme()).toBe("light");
  });

  it("reports what the registered reader says, per call", () => {
    let scheme: "dark" | "light" = "dark";
    setSystemColorSchemeReader(() => scheme);

    expect(systemColorScheme()).toBe("dark");

    scheme = "light";
    expect(systemColorScheme()).toBe("light");
  });
});
