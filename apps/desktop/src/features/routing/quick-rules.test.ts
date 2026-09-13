import { beforeEach, describe, expect, it, vi } from "vitest";

import type { RoutingRule, Routing_Serialize } from "@/ipc/bindings";

import { setQuickRule } from "./quick-rules";
import { SENTINELS, buildQuickRule } from "./sentinel-rules";

const ipc = vi.hoisted(() => ({ moveRoutingRule: vi.fn(), saveRoutingRule: vi.fn() }));
vi.mock("@/ipc/commands", () => ipc);

describe("setQuickRule", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("flips an existing rule in place and leaves a matching one alone", async () => {
    const lan = rule(SENTINELS.bypassLan);
    const current = routing([rule("Custom"), lan]);
    ipc.saveRoutingRule.mockResolvedValue(current);

    await setQuickRule(current, "bypassLan", false);
    expect(ipc.saveRoutingRule).toHaveBeenCalledWith("route", { ...lan, enabled: false });

    await setQuickRule(current, "bypassLan", true);
    expect(ipc.saveRoutingRule).toHaveBeenCalledOnce();
    expect(ipc.moveRoutingRule).not.toHaveBeenCalled();
  });

  it("does nothing when switching off a rule the profile never had", async () => {
    await setQuickRule(routing([]), "blockAds", false);

    expect(ipc.saveRoutingRule).not.toHaveBeenCalled();
  });

  it("creates a missing rule at the top, after the per-app rule when there is one", async () => {
    for (const [before, target] of [
      [[], 0],
      [[rule(SENTINELS.perApp)], 1],
    ] as Array<[RoutingRule[], number]>) {
      vi.resetAllMocks();
      const created = { ...buildQuickRule("blockAds"), id: "created" };
      const saved = routing([...before, rule("Custom"), created]);
      ipc.saveRoutingRule.mockResolvedValue(saved);
      ipc.moveRoutingRule.mockResolvedValue(saved);

      await setQuickRule(routing([...before, rule("Custom")]), "blockAds", true);

      expect(ipc.saveRoutingRule).toHaveBeenCalledWith("route", buildQuickRule("blockAds"));
      expect(ipc.moveRoutingRule).toHaveBeenCalledWith("route", "created", "position", target);
    }
  });

  it("does not move a created rule that already sits in place", async () => {
    const created = { ...buildQuickRule("bypassLan"), id: "created" };
    ipc.saveRoutingRule.mockResolvedValue(routing([created]));

    await setQuickRule(routing([]), "bypassLan", true);

    expect(ipc.moveRoutingRule).not.toHaveBeenCalled();
  });
});

function rule(remarks: string, overrides: Partial<RoutingRule> = {}): RoutingRule {
  return {
    domain: null,
    enabled: true,
    id: `id-${remarks}`,
    inboundTags: null,
    ip: null,
    kind: null,
    network: null,
    outbound: "proxy",
    port: null,
    process: null,
    protocol: null,
    remarks,
    scope: "routing",
    ...overrides,
  };
}

function routing(rules: RoutingRule[]): Routing_Serialize {
  return {
    enabled: true,
    icon: "",
    id: "route",
    isActive: true,
    locked: false,
    remarks: "Route",
    rules,
    singboxDomainStrategy: "",
    singboxRulesetPath: "",
    sort: 0,
  };
}
