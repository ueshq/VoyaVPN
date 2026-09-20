import { makeProfileEntry } from "@voya/client/mock-seed";
import type { ProfileSummaryEntry } from "@voya/contracts";
import { describe, expect, it } from "vitest";

import { homeMapMarker } from "./map-marker";

const tokyo = makeProfileEntry(0, { remarks: "🇯🇵 Tokyo" });
const measured: ProfileSummaryEntry = {
  ...tokyo,
  metrics: { ...tokyo.metrics, countryCode: "DE" },
};

function marker(overrides: Partial<Parameters<typeof homeMapMarker>[0]> = {}) {
  return homeMapMarker({
    connected: false,
    exitCountryCode: null,
    groupEntry: null,
    hasNodes: true,
    isGroup: false,
    nodeEntry: tokyo,
    ...overrides,
  });
}

describe("home map marker", () => {
  it("points at the selected node's country before connecting", () => {
    expect(marker()).toEqual({ countryCode: "JP", state: "selected" });
  });

  it("prefers the checked exit IP once connected, because that is where traffic left", () => {
    expect(marker({ connected: true, exitCountryCode: "SG" })).toEqual({
      countryCode: "SG",
      state: "connected",
    });
  });

  it("falls back to the node while the exit IP is still unknown", () => {
    // A measured country beats the flag in the name, which is only a hint.
    expect(marker({ connected: true, nodeEntry: measured })).toEqual({
      countryCode: "DE",
      state: "connected",
    });
  });

  it("marks nothing for a policy group until one of its members is running", () => {
    expect(marker({ isGroup: true })).toBeNull();
    expect(marker({ connected: true, isGroup: true, groupEntry: tokyo })).toEqual({
      countryCode: "JP",
      state: "connected",
    });
  });

  it("marks nothing without nodes, or with a node that names no country", () => {
    expect(marker({ hasNodes: false })).toBeNull();
    expect(marker({ nodeEntry: null })).toBeNull();
    expect(marker({ nodeEntry: makeProfileEntry(0, { remarks: "Home" }) })).toBeNull();
  });
});
