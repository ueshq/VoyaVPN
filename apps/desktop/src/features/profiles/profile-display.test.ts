import { describe, expect, it } from "vitest";

import type { ProfileTransport } from "@/ipc/bindings";

import { profileFlagCountryCode, profileNameWithoutFlag, profileTransportName } from "./profile-display";

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
    ["🇯🇵 Tokyo", "Tokyo"],
    [" 🇯🇵 ", " 🇯🇵 "],
    ["  Tokyo  ", "  Tokyo  "],
    ["Tokyo 🇯🇵 backup 🇺🇸", "Tokyo  backup 🇺🇸"],
  ])("preserves the node name when removing its first flag from %s", (name, expected) => {
    expect(profileNameWithoutFlag(name)).toBe(expected);
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
