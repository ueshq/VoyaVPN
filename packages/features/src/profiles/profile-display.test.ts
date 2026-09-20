import { describe, expect, it } from "vitest";

import type { TranslationFunction } from "@voya/i18n";
import type { ProfileSummaryEntry, ProfileTransport } from "@voya/contracts";
import { makeProfileFixture } from "../test/profile-fixture";

import {
  entryCountry,
  profileFlagCountryCode,
  profileLatencyTone,
  profileMemberName,
  profileNameWithoutFlag,
  profileTitle,
  profileTransportName,
} from "./profile-display";

function withMetrics(metrics: Partial<ProfileSummaryEntry["metrics"]>): ProfileSummaryEntry {
  const entry = makeProfileFixture(0, {}, false);
  return { ...entry, metrics: { ...entry.metrics, ...metrics } };
}

describe("profile display projections", () => {
  it.each([
    ["🇯🇵 Tokyo", "JP"],
    ["Backup 🇺🇸 then 🇬🇧", "US"],
    ["Tokyo", null],
    ["", null],
    [null, null],
  ] as const)("reads the country hinted by a flag in %j", (name, expected) => {
    expect(profileFlagCountryCode(name)).toBe(expected);
  });

  it.each([
    [{ delayMs: 42, outcome: null }, "good"],
    [{ delayMs: 149, outcome: "completed" }, "good"],
    [{ delayMs: 150, outcome: null }, "fair"],
    [{ delayMs: 399, outcome: null }, "fair"],
    [{ delayMs: 400, outcome: null }, "fair"],
    [{ delayMs: 0, outcome: null }, "unknown"],
    [{ delayMs: 0, outcome: "timeout" }, "poor"],
  ] as Array<[Partial<ProfileSummaryEntry["metrics"]>, string]>)(
    "bands %j as %s",
    (metrics, expected) => {
      expect(profileLatencyTone(withMetrics(metrics))).toBe(expected);
    },
  );

  it.each([
    ["🇯🇵 Tokyo", "Tokyo"],
    [" 🇯🇵 ", " 🇯🇵 "],
    ["  Tokyo  ", "  Tokyo  "],
    ["Tokyo 🇯🇵 backup 🇺🇸", "Tokyo  backup 🇺🇸"],
  ])("preserves the node name when removing its first flag from %s", (name, expected) => {
    expect(profileNameWithoutFlag(name)).toBe(expected);
  });

  it("names a member without its flag, or by its id", () => {
    expect(profileMemberName("🇯🇵 Tokyo", "node-1")).toBe("Tokyo");
    expect(profileMemberName("", "node-1")).toBe("node-1");
  });

  it("titles a node without a name as untitled", () => {
    const t = ((key: string) => `t:${key}`) as unknown as TranslationFunction;

    expect(profileTitle("🇯🇵 Tokyo", t)).toBe("🇯🇵 Tokyo");
    expect(profileTitle("", t)).toBe("t:panes.profiles.untitled");
  });

  it("prefers a measured country over the flag in the name", () => {
    const entry = makeProfileFixture(0, { remarks: "🇯🇵 Tokyo" }, false);

    expect(entryCountry({ ...entry, metrics: { ...entry.metrics, countryCode: "DE" } })).toBe("DE");
    expect(entryCountry({ ...entry, metrics: { ...entry.metrics, countryCode: null } })).toBe("JP");
    expect(entryCountry(null)).toBeNull();
  });

  it.each([
    [null, "tcp"],
    [{ header: null, host: null, kind: "tcp", path: null }, "tcp"],
    [{ host: null, kind: "websocket", path: null }, "ws"],
    [{ host: null, kind: "httpUpgrade", path: null }, "httpupgrade"],
    [{ host: null, kind: "http2", path: null }, "h2"],
    [{ authority: null, kind: "grpc", mode: null, serviceName: null }, "grpc"],
  ] as Array<[ProfileTransport | null, string]>)("maps %j to %s", (transport, expected) => {
    expect(profileTransportName(transport)).toBe(expected);
  });
});
