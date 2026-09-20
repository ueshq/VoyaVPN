import { describe, expect, it } from "vitest";

import type { SubscriptionMetadata } from "@voya/contracts";
import {
  isExpired,
  isTrafficExhausted,
  metadataBySubscriptionId,
  remainingDays,
  remainingTrafficBytes,
  usageRatio,
} from "./subscription-usage";

function metadata(overrides: Partial<SubscriptionMetadata> = {}): SubscriptionMetadata {
  return {
    subscriptionId: "sub-1",
    uploadBytes: null,
    downloadBytes: null,
    totalBytes: null,
    expireAt: null,
    lastUpdateAt: null,
    profileTitle: null,
    ...overrides,
  };
}

describe("traffic figures", () => {
  it("clamps remaining traffic at zero and requires a positive total", () => {
    expect(
      remainingTrafficBytes(metadata({ totalBytes: 1000, uploadBytes: 100, downloadBytes: 200 })),
    ).toBe(700);
    expect(remainingTrafficBytes(metadata({ totalBytes: 1000, downloadBytes: 250 }))).toBe(750);
    expect(
      remainingTrafficBytes(metadata({ totalBytes: 100, uploadBytes: 900, downloadBytes: 900 })),
    ).toBe(0);
    expect(remainingTrafficBytes(metadata({ totalBytes: 0 }))).toBeNull();
    expect(remainingTrafficBytes(metadata())).toBeNull();
  });

  it("reports usage ratio in [0, 1] only when a total exists", () => {
    expect(usageRatio(metadata({ totalBytes: 1000, downloadBytes: 250 }))).toBe(0.25);
    expect(usageRatio(metadata({ totalBytes: 100, downloadBytes: 900 }))).toBe(1);
    expect(usageRatio(metadata({ totalBytes: 1000 }))).toBe(0);
    expect(usageRatio(metadata())).toBeNull();
  });

  it("flags exhausted quotas", () => {
    expect(isTrafficExhausted(metadata({ totalBytes: 100, downloadBytes: 100 }))).toBe(true);
    expect(isTrafficExhausted(metadata({ totalBytes: 100, downloadBytes: 50 }))).toBe(false);
    expect(isTrafficExhausted(metadata())).toBe(false);
  });
});

describe("expiry", () => {
  const nowMs = Date.UTC(2026, 0, 10, 12, 0, 0);

  it("returns whole remaining days, rounding partial days up", () => {
    const inTwoAndAHalfDays = nowMs / 1000 + 2.5 * 86400;
    expect(remainingDays(inTwoAndAHalfDays, nowMs)).toBe(3);
    expect(remainingDays(nowMs / 1000 + 86400, nowMs)).toBe(1);
  });

  it("clamps past expiries to zero days and reports them expired", () => {
    const yesterday = nowMs / 1000 - 86400;
    expect(remainingDays(yesterday, nowMs)).toBe(0);
    expect(isExpired(yesterday, nowMs)).toBe(true);
    expect(isExpired(nowMs / 1000 + 60, nowMs)).toBe(false);
  });

  it("treats missing or zero expiry as no expiry", () => {
    expect(remainingDays(null, nowMs)).toBeNull();
    expect(remainingDays(0, nowMs)).toBeNull();
    expect(isExpired(null, nowMs)).toBe(false);
    expect(isExpired(0, nowMs)).toBe(false);
  });
});

describe("metadataBySubscriptionId", () => {
  it("indexes by subscription id", () => {
    const map = metadataBySubscriptionId([
      metadata({ subscriptionId: "a" }),
      metadata({ subscriptionId: "b", totalBytes: 5 }),
    ]);
    expect(map.get("b")?.totalBytes).toBe(5);
    expect(map.has("missing")).toBe(false);
  });
});
