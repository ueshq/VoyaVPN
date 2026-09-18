import { describe, expect, it } from "vitest";

import { i18next } from "@voya/i18n";

import {
  FIREWALL_KEYS,
  NAT_KEYS,
  PORT_MAPPING_KEYS,
  PROBLEM_KEYS,
  REACHABILITY_KEYS,
  REASON_KEYS,
  SELF_TEST_KEYS,
  STATUS_KEYS,
  reachabilityTone,
  selfTestTone,
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

  it("colors self-test verdicts", () => {
    expect(selfTestTone("passed")).toBe("success");
    expect(selfTestTone("failed")).toBe("danger");
    expect(selfTestTone("skipped")).toBe("secondary");
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
      FIREWALL_KEYS,
      NAT_KEYS,
      PORT_MAPPING_KEYS,
      PROBLEM_KEYS,
      REACHABILITY_KEYS,
      REASON_KEYS,
      SELF_TEST_KEYS,
      STATUS_KEYS,
    ];
    for (const registry of registries) {
      for (const key of Object.values(registry)) {
        expect(i18next.exists(key), key).toBe(true);
      }
    }
  });
});
