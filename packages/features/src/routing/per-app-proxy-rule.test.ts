import { describe, expect, it } from "vitest";

import type { RoutingRule, Routing_Serialize } from "@voya/contracts";
import {
  buildPerAppRule,
  findPerAppRule,
  normalizeProcessNames,
  readPerAppRule,
} from "./per-app-proxy-rule";

// Must match the module-private sentinel the editor writes into `remarks`.
const PER_APP_RULE_SENTINEL = "voya:per-app-proxy";

function rule(overrides: Partial<RoutingRule> = {}): RoutingRule {
  return {
    domain: null,
    enabled: true,
    id: "rule-1",
    inboundTags: null,
    ip: null,
    kind: null,
    network: null,
    outbound: "proxy",
    port: null,
    process: ["chrome.exe"],
    protocol: null,
    remarks: PER_APP_RULE_SENTINEL,
    scope: "routing",
    ...overrides,
  };
}

function routing(rules: RoutingRule[]): Routing_Serialize {
  return {
    enabled: true,
    icon: "",
    id: "routing-1",
    isActive: true,
    locked: false,
    remarks: "Active",
    rules,
    singboxDomainStrategy: "",
    singboxRulesetPath: "",
    sort: 0,
  };
}

describe("readPerAppRule", () => {
  it("reads include mode from a proxy-outbound managed rule", () => {
    expect(readPerAppRule(routing([rule()]))).toEqual({
      mode: "include",
      processes: ["chrome.exe"],
    });
  });

  it("reads exclude mode from a direct-outbound managed rule", () => {
    expect(readPerAppRule(routing([rule({ outbound: "direct" })]))).toEqual({
      mode: "exclude",
      processes: ["chrome.exe"],
    });
  });

  it("reports off when the rule is absent, disabled, or empty", () => {
    expect(readPerAppRule(routing([])).mode).toBe("off");
    expect(readPerAppRule(routing([rule({ enabled: false })])).mode).toBe("off");
    expect(readPerAppRule(routing([rule({ process: [] })])).mode).toBe("off");
    expect(readPerAppRule(null).mode).toBe("off");
  });

  it("ignores unmanaged rules that merely mention processes", () => {
    const custom = rule({ remarks: "my own rule" });
    expect(readPerAppRule(routing([custom])).mode).toBe("off");
    expect(findPerAppRule(routing([custom]))).toBeNull();
  });
});

describe("buildPerAppRule", () => {
  it("round-trips through readPerAppRule", () => {
    const built = buildPerAppRule("exclude", ["Steam", "chrome.exe"], null);
    expect(readPerAppRule(routing([built]))).toEqual({
      mode: "exclude",
      processes: ["Steam", "chrome.exe"],
    });
    expect(built.remarks).toBe(PER_APP_RULE_SENTINEL);
    expect(built.scope).toBe("routing");
  });

  it("maps include to the proxy outbound and exclude to direct", () => {
    expect(buildPerAppRule("include", ["a"], null).outbound).toBe("proxy");
    expect(buildPerAppRule("exclude", ["a"], null).outbound).toBe("direct");
  });

  it("keeps the existing rule id so saves replace in place", () => {
    expect(buildPerAppRule("include", ["a"], rule()).id).toBe("rule-1");
    expect(buildPerAppRule("include", ["a"], null).id).toBe("");
  });
});

describe("normalizeProcessNames", () => {
  it("trims, drops empties, and dedupes case-insensitively", () => {
    expect(normalizeProcessNames([" chrome.exe ", "", "Chrome.EXE", "steam"])).toEqual([
      "chrome.exe",
      "steam",
    ]);
  });
});
