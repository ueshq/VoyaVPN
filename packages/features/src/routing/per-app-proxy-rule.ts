import type { TranslationKey } from "@voya/i18n";

import type { RoutingRule, Routing_Serialize } from "@voya/contracts";

import { PER_APP_SENTINEL } from "./sentinel-rules";

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
