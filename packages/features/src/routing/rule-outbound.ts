import type { TranslationFunction, TranslationKey } from "@voya/i18n/core";

import type { ProfileSummaryEntry } from "@voya/contracts";

/**
 * The outbound tags every generated config defines, with their labels. Any
 * other outbound names a policy group as `group:<id>` or a node by its remarks.
 */
export const OUTBOUND_LABEL_KEYS = {
  proxy: "panes.routing.outboundProxy",
  direct: "panes.routing.outboundDirect",
  block: "panes.routing.outboundBlock",
} as const satisfies Record<string, TranslationKey>;

export const GROUP_OUTBOUND_PREFIX = "group:";

/** The label key for a built-in outbound tag, or `null` for any other tag. */
export function outboundLabelKey(tag: string): TranslationKey | null {
  return Object.hasOwn(OUTBOUND_LABEL_KEYS, tag)
    ? OUTBOUND_LABEL_KEYS[tag as BuiltinOutbound]
    : null;
}

/** A policy group a rule can send traffic through. */
export type RuleGroupOutbound = { id: string; name: string };

type BuiltinOutbound = keyof typeof OUTBOUND_LABEL_KEYS;

type OutboundTarget =
  | { kind: BuiltinOutbound }
  | { kind: "group" | "missing" | "missingGroup" | "node"; name: string };

function isBuiltinOutbound(value: string): value is BuiltinOutbound {
  return Object.hasOwn(OUTBOUND_LABEL_KEYS, value);
}

export function groupOutboundValue(id: string) {
  return `${GROUP_OUTBOUND_PREFIX}${id}`;
}

/**
 * The nodes a rule can send traffic to, by remarks, in list order. The
 * generator resolves a rule outbound to the first node carrying those remarks
 * and checks the built-in tags first, so duplicates and nodes named after a
 * built-in tag are not separate targets. A set, because every rule row looks
 * its outbound up in it.
 */
export function nodeOutboundNames(entries: readonly ProfileSummaryEntry[]): ReadonlySet<string> {
  const names = new Set<string>();
  for (const { profile } of entries) {
    if (
      profile.remarks.trim() &&
      !isBuiltinOutbound(profile.remarks) &&
      !profile.remarks.startsWith(GROUP_OUTBOUND_PREFIX)
    ) {
      names.add(profile.remarks);
    }
  }

  return names;
}

/**
 * Where a rule sends matching traffic. A rule without an outbound goes through
 * the proxy. `nodeNames` and `groups` are `null` until their lists have loaded,
 * and nothing is reported missing before then.
 */
/** Where a rule sends matching traffic, as one line of text for a list row. */
export function outboundText(target: OutboundTarget, t: TranslationFunction): string {
  switch (target.kind) {
    case "proxy":
    case "direct":
    case "block":
      return t(OUTBOUND_LABEL_KEYS[target.kind]);
    case "missing":
      return t("panes.routing.outboundMissing", { name: target.name });
    case "missingGroup":
      return t("panes.routing.outboundGroupMissing");
    case "group":
    case "node":
      return target.name;
  }
}

export function describeOutbound(
  outbound: string | null | undefined,
  nodeNames: ReadonlySet<string> | null,
  groups: readonly RuleGroupOutbound[] | null = [],
): OutboundTarget {
  const value = outbound?.trim() ? outbound : "proxy";
  if (isBuiltinOutbound(value)) {
    return { kind: value };
  }
  if (value.startsWith(GROUP_OUTBOUND_PREFIX)) {
    const id = value.slice(GROUP_OUTBOUND_PREFIX.length);
    if (groups === null) {
      return { kind: "group", name: id };
    }
    const group = groups.find((candidate) => candidate.id === id);
    return group ? { kind: "group", name: group.name } : { kind: "missingGroup", name: id };
  }

  return nodeNames === null || nodeNames.has(value)
    ? { kind: "node", name: value }
    : { kind: "missing", name: value };
}

/** Adds a rule-set line to a matcher list, once. */
export function appendMatcherLine(value: string, line: string) {
  const lines = value
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter(Boolean);
  return lines.includes(line) ? value : [...lines, line].join("\n");
}
