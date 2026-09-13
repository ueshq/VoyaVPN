import type { TranslationKey } from "@voya/i18n";

import type { RoutingRule, Routing_Serialize } from "@/ipc/bindings";

/**
 * Reserved remarks of the rules the Rules page manages. Mirrors
 * `voya_core::routing_seed`: the remarks are the rule's identity, so the rule
 * editor does not let users rename a managed rule.
 */
export const SENTINELS = {
  aiServices: "voya:ai-services",
  blockAds: "voya:block-ads",
  blockQuic: "voya:block-quic",
  bypassLan: "voya:bypass-lan",
  cnDirect: "voya:cn-direct",
  cnDns: "voya:cn-dns",
  perApp: "voya:per-app-proxy",
} as const;

type Sentinel = (typeof SENTINELS)[keyof typeof SENTINELS];

const SENTINEL_LABEL_KEYS = new Map<string, TranslationKey>([
  [SENTINELS.aiServices, "panes.routing.sentinel.aiServices"],
  [SENTINELS.blockAds, "panes.routing.sentinel.blockAds"],
  [SENTINELS.blockQuic, "panes.routing.sentinel.blockQuic"],
  [SENTINELS.bypassLan, "panes.routing.sentinel.bypassLan"],
  [SENTINELS.cnDirect, "panes.routing.sentinel.cnDirect"],
  [SENTINELS.cnDns, "panes.routing.sentinel.cnDns"],
  [SENTINELS.perApp, "panes.routing.sentinel.perApp"],
]);

/** A managed rule the quick settings bar switches on and off. */
export type QuickRule = "blockAds" | "bypassLan";

/** The translated name of a managed rule, or `null` for a user rule. */
export function sentinelLabelKey(remarks: string | null | undefined): TranslationKey | null {
  return remarks ? (SENTINEL_LABEL_KEYS.get(remarks) ?? null) : null;
}

export function findSentinelRule(
  routing: Routing_Serialize | null | undefined,
  remarks: Sentinel,
): RoutingRule | null {
  return routing?.rules.find((rule) => rule.remarks === remarks) ?? null;
}

export function isQuickRuleEnabled(
  routing: Routing_Serialize | null | undefined,
  rule: QuickRule,
): boolean {
  return findSentinelRule(routing, SENTINELS[rule])?.enabled ?? false;
}

/** The rule a quick setting creates when the profile has none yet. */
export function buildQuickRule(rule: QuickRule): RoutingRule {
  const base = {
    enabled: true,
    id: "",
    inboundTags: null,
    kind: null,
    network: null,
    port: null,
    process: null,
    protocol: null,
    remarks: SENTINELS[rule],
    scope: "all",
  } as const;

  return rule === "blockAds"
    ? { ...base, domain: ["geosite:category-ads-all"], ip: null, outbound: "block" }
    : { ...base, domain: ["geosite:private"], ip: ["geoip:private"], outbound: "direct" };
}
