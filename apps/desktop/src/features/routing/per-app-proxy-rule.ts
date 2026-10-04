import type { TranslationKey } from "@voya/i18n/core";

import type { RoutingRule, Routing_Serialize } from "@voya/contracts";

import { voyaCommands } from "@voya/client/transport";
import { PER_APP_SENTINEL } from "@voya/features/routing/sentinel-rules";

/**
 * `include`: the listed apps always go through the proxy (ahead of other
 * rules). `exclude`: the listed apps always bypass the proxy. `off`: no
 * managed rule.
 */
export type PerAppProxyMode = "exclude" | "include" | "off";

export const PER_APP_MODE_LABEL_KEYS = {
  exclude: "panes.routing.perAppModeExclude",
  include: "panes.routing.perAppModeInclude",
  off: "panes.routing.perAppModeOff",
} as const satisfies Record<PerAppProxyMode, TranslationKey>;

type PerAppProxyState = {
  mode: PerAppProxyMode;
  processes: string[];
};

export function findPerAppRule(routing: Routing_Serialize | null | undefined): RoutingRule | null {
  return routing?.rules.find((rule) => rule.remarks === PER_APP_SENTINEL) ?? null;
}

export function readPerAppRule(routing: Routing_Serialize | null | undefined): PerAppProxyState {
  const rule = findPerAppRule(routing);
  const processes = rule?.process ?? [];
  if (!rule || !rule.enabled || processes.length === 0) {
    return { mode: "off", processes };
  }

  return { mode: rule.outbound === "direct" ? "exclude" : "include", processes };
}

/**
 * Builds the managed rule for a non-off mode, reusing the existing rule's id
 * so a save replaces it in place.
 */
export function buildPerAppRule(
  mode: Exclude<PerAppProxyMode, "off">,
  processes: string[],
  existing: RoutingRule | null,
): RoutingRule {
  return {
    domain: null,
    enabled: true,
    id: existing?.id ?? "",
    inboundTags: null,
    ip: null,
    kind: null,
    network: null,
    outbound: mode === "exclude" ? "direct" : "proxy",
    port: null,
    process: normalizeProcessNames(processes),
    protocol: null,
    remarks: PER_APP_SENTINEL,
    scope: "routing",
  };
}

/**
 * Saves the dialog's choice as the routing's managed rule.
 *
 * Turning it off keeps the chosen apps on a disabled rule, so turning it back
 * on does not mean picking them all again; with no apps left there is nothing
 * to keep and the rule goes.
 *
 * Turning it on pins the rule to the top. The backend appends a new rule to
 * the end of the rule set, behind the catch-all every built-in routing ends
 * with, where a process rule can never match.
 */
export async function savePerAppRule(
  routing: Routing_Serialize,
  mode: PerAppProxyMode,
  selected: readonly string[],
): Promise<void> {
  const existing = findPerAppRule(routing);
  const processes = normalizeProcessNames(selected);
  if (mode === "off") {
    if (existing && processes.length > 0) {
      await voyaCommands().saveRoutingRule(routing.id, {
        ...existing,
        enabled: false,
        process: processes,
      });
    } else if (existing) {
      await voyaCommands().deleteRoutingRules(routing.id, [existing.id]);
    }
    return;
  }

  const saved = await voyaCommands().saveRoutingRule(
    routing.id,
    buildPerAppRule(mode, processes, existing),
  );
  const savedRule = findPerAppRule(saved);
  if (savedRule && saved.rules[0]?.id !== savedRule.id) {
    await voyaCommands().moveRoutingRule(saved.id, savedRule.id, "top", null);
  }
}

/** Trims, drops empties, and dedupes case-insensitively preserving order. */
export function normalizeProcessNames(processes: readonly string[]): string[] {
  const seen = new Set<string>();
  const normalized: string[] = [];
  for (const process of processes) {
    const trimmed = process.trim();
    const key = trimmed.toLowerCase();
    if (trimmed.length === 0 || seen.has(key)) {
      continue;
    }
    seen.add(key);
    normalized.push(trimmed);
  }

  return normalized;
}
