import type { RoutingRule, RoutingRuleScope } from "@/ipc/bindings";

export type MatchListField = "domain" | "ip" | "process" | "protocol";

/**
 * One compact fact about what a rule matches: the first value of a matcher
 * list plus how many more it holds, the network and port, or a scope narrower
 * than routing and DNS together.
 */
export type MatchChip =
  | { field: MatchListField; first: string; kind: "list"; more: number }
  | { kind: "port"; network: string | null; port: string | null }
  | { kind: "scope"; scope: Exclude<RoutingRuleScope, "all"> };

const LIST_FIELDS: readonly MatchListField[] = ["domain", "ip", "process", "protocol"];

type MatcherFields = Pick<
  RoutingRule,
  "domain" | "inboundTags" | "ip" | "network" | "port" | "process" | "protocol"
>;

export function ruleMatchChips(rule: RoutingRule): MatchChip[] {
  const chips: MatchChip[] = [];
  for (const field of LIST_FIELDS) {
    const values = nonEmpty(rule[field]);
    if (values.length > 0) {
      chips.push({ field, first: values[0], kind: "list", more: values.length - 1 });
    }
  }

  const network =
    (rule.network ?? "")
      .split(",")
      .map((value) => value.trim().toUpperCase())
      .filter(Boolean)
      .join("/") || null;
  const port = rule.port?.trim() || null;
  if (network || port) {
    chips.push({ kind: "port", network, port });
  }

  if (rule.scope === "routing" || rule.scope === "dns") {
    chips.push({ kind: "scope", scope: rule.scope });
  }

  return chips;
}

/**
 * Whether the generator emits anything for the rule. A rule with no matcher at
 * all is skipped, so it never matches any traffic.
 */
export function ruleHasMatcher(rule: MatcherFields): boolean {
  return (
    [rule.domain, rule.ip, rule.process, rule.protocol, rule.inboundTags].some(
      (values) => nonEmpty(values).length > 0,
    ) ||
    Boolean(rule.port?.trim()) ||
    Boolean(rule.network?.trim())
  );
}

function nonEmpty(values: readonly string[] | null | undefined): string[] {
  return (values ?? []).filter((value) => value.trim().length > 0);
}
