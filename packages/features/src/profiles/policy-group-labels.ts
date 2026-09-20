import type { TranslationKey } from "@voya/i18n";
import type { PolicyGroupStrategy } from "@voya/contracts";

/** Offered in this order: automatic choices first, manual selection last. */
export const POLICY_GROUP_STRATEGIES = [
  "urlTest",
  "fallback",
  "selector",
] as const satisfies readonly PolicyGroupStrategy[];

export const POLICY_GROUP_STRATEGY_KEYS: Record<PolicyGroupStrategy, TranslationKey> = {
  fallback: "policyGroups.strategy.fallback",
  selector: "policyGroups.strategy.selector",
  urlTest: "policyGroups.strategy.urlTest",
};

export const POLICY_GROUP_STRATEGY_HINT_KEYS: Record<PolicyGroupStrategy, TranslationKey> = {
  fallback: "policyGroups.strategyHint.fallback",
  selector: "policyGroups.strategyHint.selector",
  urlTest: "policyGroups.strategyHint.urlTest",
};
