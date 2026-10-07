import { beforeEach, describe, expect, it } from "vite-plus/test";

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
      backend: "macosPacketTunnel",
      lastProviderError: "the tunnel did not come up within 60s",
      providerState: "error",
    });

    expect(homeTunIssue(failed, t())).toBe("macOS PacketTunnel: Error: the tunnel did not come up within 60s");
    expect(homeTunIssue(failed, t(), { includeProviderError: false })).toBe("macOS PacketTunnel: Error");
  });

  // "iOS PacketTunnel: Error" names a component, not what to do; the review
  // found it on Home after a failed connect. The provider's text stays in
  // the full line, which the phone tucks behind its diagnostics.
  it("says what to check when a phone's tunnel fails, leaving the details to diagnostics", () => {
    const failed = (backend: TunStatus["backend"]) =>
      tun({ backend, lastProviderError: "the tunnel did not come up within 60s", providerState: "error" });

    expect(homeTunIssue(failed("iosPacketTunnel"), t(), { includeProviderError: false })).toBe(i18next.t("home.vpnErrorIos"));
    expect(homeTunIssue(failed("androidVpnService"), t(), { includeProviderError: false })).toBe(i18next.t("home.vpnErrorAndroid"));
    expect(homeTunIssue(failed("iosPacketTunnel"), t())).toBe("iOS PacketTunnel: Error: the tunnel did not come up within 60s");
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

  // Connect-time complaints wait for something to connect: after the selected
  // node is deleted, a stale authorization failure must not outlive it on the
  // Home screen. An installation-level path mismatch stays stated regardless.
  it("holds connect-time complaints while nothing is selected, but keeps path mismatches", () => {
    const denied = tun({
      backend: "iosPacketTunnel",
      providerState: "permissionRequired",
      lastProviderError: "the system did not authorize the VPN configuration",
    });

    expect(homeTunIssue(denied, t(), { ready: false })).toBeNull();
    expect(homeTunIssue(denied, t(), { ready: true })).toContain("Settings → VPN");

    const mismatch = tun({
      backend: "macosPacketTunnel",
      expectedProviderPath: "/Applications/VoyaVPN.app",
      providerPathMismatch: true,
      providerState: "error",
      resolvedProviderPath: "/tmp/VoyaVPN.app",
    });
    expect(homeTunIssue(mismatch, t(), { ready: false })).toContain("/tmp/VoyaVPN.app");
  });
});
