import { ruleHasMatcher } from "./rule-match-summary";
import type { RoutingRule } from "@voya/contracts";
import type { TranslationFunction } from "@voya/i18n/core";

/** Keep matcher values exact: a domain token may be a suffix, regexp or rule-set reference. */
export function ruleMatchSummary(rule: RoutingRule, t: TranslationFunction): string[] {
  if (!ruleHasMatcher(rule)) return [t("panes.routing.matchNothing")];
  const lines: string[] = [];
  if (rule.domain?.length) lines.push(`${t("panes.routing.domain")}: ${rule.domain.join(", ")}`);
  if (rule.ip?.length) lines.push(t("daily.ruleIps", { value: rule.ip.join(", ") }));
  if (rule.port) lines.push(`${t("panes.routing.port")}: ${rule.port}`);
  if (rule.network) lines.push(`${t("panes.routing.network")}: ${rule.network}`);
  if (rule.protocol?.length) lines.push(`${t("panes.routing.protocol")}: ${rule.protocol.join(", ")}`);
  if (rule.process?.length) lines.push(`${t("panes.routing.process")}: ${rule.process.join(", ")}`);
  if (rule.inboundTags?.length) lines.push(t("daily.ruleInbounds", { value: rule.inboundTags.join(", ") }));
  if (rule.kind) lines.push(t("daily.ruleKind", { value: rule.kind }));
  return lines;
}
