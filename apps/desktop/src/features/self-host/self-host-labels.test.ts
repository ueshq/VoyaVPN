import { describe, expect, it } from "vitest";

import { i18next } from "@voya/i18n";

import type { SelfHostFamilyReport, SelfHostReachability } from "@voya/contracts";

import {
  ADDRESS_KIND_KEYS,
  PROBLEM_KEYS,
  REACHABILITY_KEYS,
  REASON_KEYS,
  STATUS_HINT_KEYS,
  STATUS_KEYS,
  VERDICT_KEYS,
  actionableReasons,
  overallReachability,
  reachabilityTone,
  statusTone,
} from "./self-host-labels";

describe("self-hosted node labels", () => {
  it("colors reachability by how much a peer can rely on it", () => {
    expect(reachabilityTone("reachable")).toBe("success");
    expect(reachabilityTone("likelyReachable")).toBe("warning");
    expect(reachabilityTone("needsPortForward")).toBe("warning");
    expect(reachabilityTone("unreachable")).toBe("danger");
    expect(reachabilityTone("noConnectivity")).toBe("secondary");
    expect(reachabilityTone("unknown")).toBe("secondary");
  });

  it("lets the better address family decide the overall verdict", () => {
    const report = (ipv4: SelfHostReachability, ipv6: SelfHostReachability) =>
      overallReachability({ ipv4: family(ipv4), ipv6: family(ipv6) });

    // A peer needs one working family, so IPv6 alone is enough.
    expect(report("unreachable", "reachable")).toBe("reachable");
    expect(report("needsPortForward", "likelyReachable")).toBe("likelyReachable");
    // A tested failure says more than a family that was never online.
    expect(report("unreachable", "noConnectivity")).toBe("unreachable");
    expect(report("noConnectivity", "unknown")).toBe("unknown");
    expect(report("noConnectivity", "noConnectivity")).toBe("noConnectivity");
  });

  it("lists each finding once and leaves out the ones that need nothing done", () => {
    const reasons = actionableReasons({
      ipv4: {
        ...family("needsPortForward"),
        reasons: ["behindNat", "portMapped", "upnpUnavailable", "firewallRuleMissing"],
      },
      ipv6: {
        ...family("noConnectivity"),
        reasons: ["noPublicAddress", "behindNat", "ipv6FirewallUnknown", "probeReachable", "publicAddressOnDevice"],
      },
    });

    // The port-forward verdict already says the device is behind a router.
    expect(reasons).toEqual(["upnpUnavailable", "ipv6FirewallUnknown"]);
  });

  it("keeps the router finding when the verdict does not say it", () => {
    const reasons = actionableReasons({
      ipv4: { ...family("unreachable"), reasons: ["behindNat", "probeTimedOut"] },
      ipv6: family("noConnectivity"),
    });

    expect(reasons).toEqual(["behindNat", "probeTimedOut"]);
  });

  it("maps every runtime status onto a dot state", () => {
    expect(statusTone("running")).toBe("on");
    expect(statusTone("starting")).toBe("busy");
    expect(statusTone("retrying")).toBe("busy");
    expect(statusTone("stopped")).toBe("off");
    expect(statusTone("failed")).toBe("error");
  });

  it("gives every code a translated sentence", () => {
    const registries = [
      ADDRESS_KIND_KEYS,
      PROBLEM_KEYS,
      REACHABILITY_KEYS,
      REASON_KEYS,
      STATUS_HINT_KEYS,
      STATUS_KEYS,
      VERDICT_KEYS,
    ];
    for (const registry of registries) {
      for (const key of Object.values(registry)) {
        expect(i18next.exists(key), key).toBe(true);
      }
    }
  });
});

function family(reachability: SelfHostReachability): SelfHostFamilyReport {
  return {
    family: "ipv4",
    nat: "unknown",
    publicAddress: null,
    reachability,
    reasons: [],
    verifiedByProbe: false,
  };
}
