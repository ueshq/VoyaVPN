import type { ProxyGroup } from "@/ipc/bindings";

const AUTO_GROUP_TYPES = new Set(["urltest", "fallback"]);

/** Delay-managed auto-select groups (sing-box `urltest` / `fallback`). */
export function isAutoGroup(group: ProxyGroup): boolean {
  return AUTO_GROUP_TYPES.has(group.proxyType.toLowerCase());
}

/**
 * Pins auto-select groups first (Hiddify's "Auto" node placement) while
 * keeping the backend order stable within each partition.
 */
export function orderProxyGroups(groups: readonly ProxyGroup[]): ProxyGroup[] {
  return [...groups.filter(isAutoGroup), ...groups.filter((group) => !isAutoGroup(group))];
}
