import type { TranslationFunction, TranslationKey } from "@voya/i18n/core";

import type { RoutingRule } from "@voya/contracts";

/** Reserved remarks of the per-app proxy rule, which the per-app dialog owns. */
export const PER_APP_SENTINEL = "voya:per-app-proxy";

/** Reserved remarks of the rule that blocks QUIC so clients fall back to TCP. */
export const SENTINEL_BLOCK_QUIC = "voya:block-quic";

/**
 * Names of the rules VoyaVPN manages, keyed by their reserved remarks. Mirrors
 * `voya_core::routing_seed`: the remarks are the rule's identity, so the rule
 * editor does not let users rename a managed rule.
 */
const SENTINEL_LABEL_KEYS = new Map<string, TranslationKey>([
  ["voya:ai-services", "panes.routing.sentinel.aiServices"],
  ["voya:block-ads", "panes.routing.sentinel.blockAds"],
  [SENTINEL_BLOCK_QUIC, "panes.routing.sentinel.blockQuic"],
  ["voya:bypass-lan", "panes.routing.sentinel.bypassLan"],
  ["voya:cn-direct", "panes.routing.sentinel.cnDirect"],
  ["voya:cn-dns", "panes.routing.sentinel.cnDns"],
  [PER_APP_SENTINEL, "panes.routing.sentinel.perApp"],
]);

/** The translated name of a managed rule, or `null` for a user rule. */
export function sentinelLabelKey(remarks: string | null | undefined): TranslationKey | null {
  return remarks ? (SENTINEL_LABEL_KEYS.get(remarks) ?? null) : null;
}

/** What the page calls a rule: a managed rule's translated name, else its remarks. */
export function ruleDisplayName(
  rule: Pick<RoutingRule, "remarks">,
  t: TranslationFunction,
): string {
  const labelKey = sentinelLabelKey(rule.remarks);
  if (labelKey) {
    return t(labelKey);
  }

  return rule.remarks?.trim() ? rule.remarks : t("panes.routing.untitled");
}
