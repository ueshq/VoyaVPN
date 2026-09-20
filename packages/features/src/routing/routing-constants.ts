import type { TranslationKey } from "@voya/i18n";

import type { RoutingRuleScope } from "@voya/contracts";

/** Where a rule applies, in the order the editor offers it. */
export const RULE_SCOPE_LABEL_KEYS = {
  all: "panes.routing.scopeBoth",
  routing: "panes.routing.scopeRoutingOnly",
  dns: "panes.routing.scopeDnsOnly",
} as const satisfies Record<RoutingRuleScope, TranslationKey>;
