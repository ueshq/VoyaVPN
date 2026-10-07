import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import type { RoutingRule, Routing_Serialize } from "@voya/contracts";
import { installFakeCommands } from "@voya/features/test/backend";
import {
  buildPerAppRule,
  findPerAppRule,
  normalizeProcessNames,
  readPerAppRule,
  savePerAppRule,
} from "./per-app-proxy-rule";

const ipc = installFakeCommands({
  deleteRoutingRules: vi.fn(),
  moveRoutingRule: vi.fn(),
  saveRoutingRule: vi.fn(),
});

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
    id: "routing-1",
    isActive: true,
    remarks: "Active",
    rules,
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

describe("savePerAppRule", () => {
  const other = rule({ id: "rule-other", process: null, remarks: "my own rule" });

  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("turning it off keeps the chosen apps on a disabled rule", async () => {
    const existing = rule();
    await savePerAppRule(routing([existing]), "off", ["chrome.exe", " Steam "]);

    expect(ipc.saveRoutingRule).toHaveBeenCalledExactlyOnceWith("routing-1", {
      ...existing,
      enabled: false,
      process: ["chrome.exe", "Steam"],
    });
    expect(ipc.deleteRoutingRules).not.toHaveBeenCalled();
  });

  it("turning it off with no apps left deletes the rule, and without a rule does nothing", async () => {
    await savePerAppRule(routing([rule()]), "off", []);
    expect(ipc.deleteRoutingRules).toHaveBeenCalledExactlyOnceWith("routing-1", ["rule-1"]);

    vi.resetAllMocks();
    await savePerAppRule(routing([other]), "off", ["chrome.exe"]);
    expect(ipc.saveRoutingRule).not.toHaveBeenCalled();
    expect(ipc.deleteRoutingRules).not.toHaveBeenCalled();
  });

  it("turning it on saves the rule and pins it above the catch-all", async () => {
    // The backend appends a new rule, which lands behind everything else.
    const saved = rule({ id: "rule-new", outbound: "direct" });
    ipc.saveRoutingRule.mockResolvedValue(routing([other, saved]));

    await savePerAppRule(routing([other]), "exclude", ["chrome.exe"]);

    expect(ipc.saveRoutingRule).toHaveBeenCalledExactlyOnceWith(
      "routing-1",
      buildPerAppRule("exclude", ["chrome.exe"], null),
    );
    expect(ipc.moveRoutingRule).toHaveBeenCalledExactlyOnceWith(
      "routing-1",
      "rule-new",
      "top",
      null,
    );
  });

  it("leaves a rule that is already first where it is", async () => {
    const existing = rule();
    ipc.saveRoutingRule.mockResolvedValue(routing([existing, other]));

    await savePerAppRule(routing([existing, other]), "include", ["chrome.exe"]);

    expect(ipc.moveRoutingRule).not.toHaveBeenCalled();
  });
});
