import type { Routing_Serialize } from "@/ipc/bindings";
import { moveRoutingRule, saveRoutingRule } from "@/ipc/commands";

import { SENTINELS, buildQuickRule, findSentinelRule, type QuickRule } from "./sentinel-rules";

/**
 * Switches a quick rule on or off in `routing`.
 *
 * An existing rule only flips `enabled`, so its position and any edits the
 * user made survive. A missing rule is created ahead of every other rule except
 * the per-app rule, which has to stay first to match at all.
 */
export async function setQuickRule(
  routing: Routing_Serialize,
  rule: QuickRule,
  enabled: boolean,
): Promise<Routing_Serialize> {
  const existing = findSentinelRule(routing, SENTINELS[rule]);
  if (existing) {
    return existing.enabled === enabled
      ? routing
      : saveRoutingRule(routing.id, { ...existing, enabled });
  }
  if (!enabled) {
    return routing;
  }

  const saved = await saveRoutingRule(routing.id, buildQuickRule(rule));
  const created = findSentinelRule(saved, SENTINELS[rule]);
  if (!created) {
    return saved;
  }
  const target = findSentinelRule(saved, SENTINELS.perApp) ? 1 : 0;

  return saved.rules.indexOf(created) === target
    ? saved
    : moveRoutingRule(saved.id, created.id, "position", target);
}
