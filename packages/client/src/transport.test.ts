import { describe, expect, it, vi } from "vitest";

import type { VoyaCommands } from "@voya/contracts";

import { setVoyaCommands, voyaCommands } from "./transport";

describe("voyaCommands", () => {
  it("refuses to guess when no platform registered itself", () => {
    // A missing registration is a wiring bug at startup, not a state a feature
    // should have to handle, so this throws rather than returning null.
    expect(() => voyaCommands()).toThrow(/No VoyaCommands registered/);
  });

  it("returns whatever the platform registered", () => {
    const platform = { restartCore: vi.fn() } as unknown as VoyaCommands;

    setVoyaCommands(platform);

    expect(voyaCommands()).toBe(platform);
  });
});
