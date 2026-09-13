import type { TranslationKey } from "@voya/i18n";

import type { ProfileListEntry } from "@/ipc/bindings";

/**
 * The outbound tags every generated config defines, with their labels. Any
 * other outbound names a node by its remarks.
 */
export const OUTBOUND_LABEL_KEYS = {
  proxy: "panes.routing.outboundProxy",
  direct: "panes.routing.outboundDirect",
  block: "panes.routing.outboundBlock",
} as const satisfies Record<string, TranslationKey>;

type BuiltinOutbound = keyof typeof OUTBOUND_LABEL_KEYS;

export type OutboundTarget = { kind: BuiltinOutbound } | { kind: "missing" | "node"; name: string };

function isBuiltinOutbound(value: string): value is BuiltinOutbound {
  return Object.hasOwn(OUTBOUND_LABEL_KEYS, value);
}

/**
 * The nodes a rule can send traffic to, by remarks, in list order. The
 * generator resolves a rule outbound to the first node carrying those remarks
 * and checks the built-in tags first, so duplicates and nodes named after a
 * built-in tag are not separate targets.
 */
export function nodeOutboundNames(entries: readonly ProfileListEntry[]): string[] {
  const names = new Set<string>();
  for (const { profile } of entries) {
    if (profile.remarks.trim() && !isBuiltinOutbound(profile.remarks)) {
      names.add(profile.remarks);
    }
  }

  return [...names];
}

/**
 * Where a rule sends matching traffic. A rule without an outbound goes through
 * the proxy. `nodeNames` is `null` until the node list has loaded, and nothing
 * is reported missing before then.
 */
export function describeOutbound(
  outbound: string | null | undefined,
  nodeNames: readonly string[] | null,
): OutboundTarget {
  const value = outbound?.trim() ? outbound : "proxy";
  if (isBuiltinOutbound(value)) {
    return { kind: value };
  }

  return nodeNames === null || nodeNames.includes(value)
    ? { kind: "node", name: value }
    : { kind: "missing", name: value };
}
