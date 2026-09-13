import { describe, expect, it } from "vitest";

import type { RoutingRule, Routing_Serialize } from "@/ipc/bindings";

import {
  SENTINELS,
  buildQuickRule,
  findSentinelRule,
  isQuickRuleEnabled,
  sentinelLabelKey,
} from "./sentinel-rules";

describe("sentinel rules", () => {
  it("labels every managed rule and nothing else", () => {
    for (const remarks of Object.values(SENTINELS)) {
      expect(sentinelLabelKey(remarks)).toMatch(/^panes\.routing\.sentinel\./);
    }
    expect(sentinelLabelKey("voya:unknown")).toBeNull();
    expect(sentinelLabelKey("toString")).toBeNull();
    expect(sentinelLabelKey(null)).toBeNull();
  });

  it("reads a quick rule's state from its enabled flag", () => {
    const route = routing([{ ...buildQuickRule("bypassLan"), enabled: false, id: "lan" }]);

    expect(findSentinelRule(route, SENTINELS.bypassLan)?.id).toBe("lan");
    expect(isQuickRuleEnabled(route, "bypassLan")).toBe(false);
    expect(isQuickRuleEnabled(route, "blockAds")).toBe(false);
    expect(isQuickRuleEnabled(null, "blockAds")).toBe(false);
  });

  it("builds quick rules in the shapes the seed uses", () => {
    expect(buildQuickRule("blockAds")).toMatchObject({
      domain: ["geosite:category-ads-all"],
      outbound: "block",
      remarks: SENTINELS.blockAds,
      scope: "all",
    });
    expect(buildQuickRule("bypassLan")).toMatchObject({
      domain: ["geosite:private"],
      ip: ["geoip:private"],
      outbound: "direct",
      remarks: SENTINELS.bypassLan,
    });
  });
});

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
