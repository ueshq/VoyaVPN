import type { TranslationFunction } from "@voya/i18n";
import { ruleDisplayName } from "@/features/routing/sentinel-rules";
import type { ProxyConnectionItem, RoutingRule } from "@/ipc/bindings";

// What the core reports for a connection that matched no route rule.
const FINAL_RULES = new Set(["final", "match"]);
// Only these are matcher prefixes; an IPv6 address has colons of its own.
const MATCHER_PREFIXES = new Set(["domain", "full", "keyword", "regexp", "geosite", "geoip", "ext"]);

/**
 * The words a generated route rule carries for a rule's matchers, the way the
 * generator writes them: `geosite:cn` becomes the rule set `geosite-cn` and
 * `domain:example.com` the suffix `example.com`.
 */
function matcherTokens(rule: RoutingRule): string[] {
  const tokens: string[] = [];
  for (const value of [...(rule.domain ?? []), ...(rule.ip ?? [])]) {
    const trimmed = value.trim();
    const colon = trimmed.indexOf(":");
    const prefix = colon > 0 ? trimmed.slice(0, colon) : "";
    if (!MATCHER_PREFIXES.has(prefix)) {
      tokens.push(trimmed);
    } else if (prefix === "geosite" || prefix === "geoip") {
      tokens.push(`${prefix}-${trimmed.slice(colon + 1)}`);
    } else {
      tokens.push(trimmed.slice(colon + 1));
    }
  }
  tokens.push(...(rule.process ?? []).map((value) => value.trim()));
  return tokens.filter(Boolean).map((token) => token.toLowerCase());
}

/**
 * Names the rule a connection matched in the Rules page's own words. The core
 * only reports the generated conditions, which carry no rule name, so the
 * first enabled rule with a matcher among those conditions is taken to be the
 * one; the core's own text stays underneath.
 */
export function connectionRuleText(
  connection: Pick<ProxyConnectionItem, "rule" | "rulePayload">,
  rules: readonly RoutingRule[] | null,
  t: TranslationFunction,
): string {
  const rule = connection.rule?.trim() ?? "";
  if (FINAL_RULES.has(rule.toLowerCase())) return t("activity.ruleFinal");
  const raw = [rule, connection.rulePayload?.trim()].filter(Boolean).join(" · ");
  if (!raw) return "";
  const words = new Set(
    raw
      .toLowerCase()
      .split(/[\s[\]=,()"'>·]+/)
      .filter(Boolean),
  );
  const matched = rules?.find(
    (item) => item.enabled && matcherTokens(item).some((token) => words.has(token)),
  );
  return matched ? `${ruleDisplayName(matched, t)}\n${raw}` : raw;
}
