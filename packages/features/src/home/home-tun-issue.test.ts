import { beforeEach, describe, expect, it } from "vitest";

import { makeMockSeed } from "@voya/client/mock-seed";
import type { TunStatus } from "@voya/contracts";
import { changeLocale, i18next } from "@voya/i18n";

import { homeTunIssue } from "./use-home-runtime";

function tun(overrides: Partial<TunStatus>): TunStatus {
  return { ...makeMockSeed().tun, providerPathMismatch: false, lastProviderError: null, ...overrides };
}

describe("homeTunIssue", () => {
  beforeEach(async () => {
    await changeLocale("en", { persist: false });
  });

  const t = () => i18next.t.bind(i18next);

  it("says nothing while the tunnel is healthy or not in use", () => {
    expect(homeTunIssue(null, t())).toBeNull();
    expect(homeTunIssue(tun({ backend: "iosPacketTunnel", providerState: "running" }), t())).toBeNull();
    expect(homeTunIssue(tun({ backend: "iosPacketTunnel", providerState: "notApplicable" }), t())).toBeNull();
  });

  // Each system words its first-connection prompt differently; the hint names the right button.
  it("explains the system's VPN prompt on every platform that has one", () => {
    const hint = (backend: TunStatus["backend"]) => homeTunIssue(tun({ backend, providerState: "permissionRequired" }), t());

    expect(hint("macosPacketTunnel")).toBe(i18next.t("home.vpnPermissionHint"));
    expect(hint("iosPacketTunnel")).toBe(i18next.t("home.vpnPermissionHintIos"));
    expect(hint("iosPacketTunnel")).toContain("Settings → VPN");
    expect(hint("androidVpnService")).toBe(i18next.t("home.vpnPermissionHintAndroid"));
    // A backend with no system prompt falls back to the state line.
    expect(hint("process")).toBe("VPN process: Permission required");
  });

  it("keeps the provider's own error in the line unless asked to leave it out", () => {
    const failed = tun({
      backend: "iosPacketTunnel",
      lastProviderError: "the tunnel did not come up within 60s",
      providerState: "error",
    });

    expect(homeTunIssue(failed, t())).toBe("iOS PacketTunnel: Error: the tunnel did not come up within 60s");
    expect(homeTunIssue(failed, t(), { includeProviderError: false })).toBe("iOS PacketTunnel: Error");
  });

  it("reports a provider running from another copy of the app first", () => {
    const mismatch = tun({
      backend: "macosPacketTunnel",
      expectedProviderPath: "/Applications/VoyaVPN.app",
      providerPathMismatch: true,
      providerState: "permissionRequired",
      resolvedProviderPath: "/tmp/VoyaVPN.app",
    });

    expect(homeTunIssue(mismatch, t())).toContain("/tmp/VoyaVPN.app");
  });
});
