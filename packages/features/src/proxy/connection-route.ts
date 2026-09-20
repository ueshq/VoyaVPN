import type { ProxyConnectionItem } from "@voya/contracts";

export type ConnectionRoute =
  | { kind: "block" }
  | { kind: "direct" }
  | { kind: "proxy"; node: string | null }
  | { kind: "unknown" };

const BLOCK_TAGS = new Set(["block", "reject"]);
// Outbounds the generator names itself; any other tag is a node or a policy group.
const BUILT_IN_TAGS = new Set(["proxy", "direct", "block", "reject", "global"]);

/**
 * Where a connection went, read from the core's outbound chain (entry first,
 * as the details dialog shows it): blocked, straight out, or through the proxy
 * and, when the chain names one, the node it finally left from.
 */
export function connectionRoute(connection: Pick<ProxyConnectionItem, "chains">): ConnectionRoute {
  const tags = connection.chains.map((tag) => tag.trim()).filter(Boolean);
  if (!tags.length) return { kind: "unknown" };
  const lower = tags.map((tag) => tag.toLowerCase());
  if (lower.some((tag) => BLOCK_TAGS.has(tag))) return { kind: "block" };
  if (lower.includes("direct")) return { kind: "direct" };
  const node = tags.findLast((tag) => !BUILT_IN_TAGS.has(tag.toLowerCase())) ?? null;
  return { kind: "proxy", node };
}
