import type { RoutingRuleScope } from "@/ipc/bindings";

export const RULE_TYPES = {
  All: "all",
  Routing: "routing",
  Dns: "dns",
} as const satisfies Record<string, RoutingRuleScope>;

export const DOMAIN_STRATEGIES = ["AsIs", "IPIfNonMatch", "IPOnDemand"] as const;

export const SINGBOX_DOMAIN_STRATEGIES = ["", "prefer_ipv4", "prefer_ipv6", "ipv4_only", "ipv6_only"] as const;
