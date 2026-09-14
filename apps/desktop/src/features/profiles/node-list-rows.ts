import type { ProfileListEntry, Subscription } from "@/ipc/bindings";

export const LOCAL_GROUP_KEY = "local";
export type NodeSourceKey = typeof LOCAL_GROUP_KEY | `subscription:${string}`;
type GroupBoundary = { groupKey: NodeSourceKey; last: boolean };
export type NodeListRow = GroupBoundary &
  (
    | { kind: "profile"; key: string; item: ProfileListEntry }
    | {
        kind: "group";
        key: NodeSourceKey;
        subscription: Subscription | null;
        name: string;
        /** The members the list shows, after the view's order and filter. */
        members: ProfileListEntry[];
        /** Every member, so a group test also retries hidden unreachable nodes. */
        allMembers: ProfileListEntry[];
        expanded: boolean;
      }
  );

export type NodeListView = { hideUnreachable?: boolean; sortByLatency?: boolean; search?: string };

// Outcomes that mean the node could not be reached when last tested.
const UNREACHABLE_OUTCOMES: ReadonlySet<string> = new Set([
  "timedOut",
  "proxyConnectFailed",
  "proxyConnectionRefused",
  "proxyConnectionClosed",
  "invalidProfile",
]);

function measuredLatency(item: ProfileListEntry) {
  return item.metrics.outcome === "completed" && item.metrics.delayMs > 0
    ? item.metrics.delayMs
    : Number.POSITIVE_INFINITY;
}

/** Untested nodes stay visible; sorting puts measured nodes first, fastest first. */
function arrangeMembers(members: ProfileListEntry[], view: NodeListView) {
  const shown = view.hideUnreachable
    ? members.filter((item) => !UNREACHABLE_OUTCOMES.has(item.metrics.outcome ?? ""))
    : members;
  if (!view.sortByLatency) return shown;
  return shown.toSorted((a, b) => {
    const left = measuredLatency(a);
    const right = measuredLatency(b);
    return left === right ? 0 : left - right;
  });
}

/** Source ownership is the only grouping authority. Names never identify groups. */
export function profilesByNodeGroup(profiles: readonly ProfileListEntry[]) {
  const byGroup = new Map<NodeSourceKey, ProfileListEntry[]>();
  for (const item of profiles) {
    const key: NodeSourceKey = item.profile.subscriptionId
      ? `subscription:${item.profile.subscriptionId}`
      : LOCAL_GROUP_KEY;
    const members = byGroup.get(key) ?? [];
    members.push(item);
    byGroup.set(key, members);
  }
  return byGroup;
}

export function nodeListRows(
  profiles: ProfileListEntry[],
  collapsed: ReadonlySet<string>,
  localName: string,
  subscriptions: readonly Subscription[] = [],
  unknownName: string,
  view: NodeListView = {},
): NodeListRow[] {
  const byGroup = profilesByNodeGroup(profiles);
  const needle = view.search?.trim().toLocaleLowerCase() ?? "";
  const rows: NodeListRow[] = [];
  function append(
    groupKey: NodeSourceKey,
    name: string,
    subscription: Subscription | null,
  ) {
    const allMembers = byGroup.get(groupKey) ?? [];
    const matched = needle
      ? allMembers.filter(({ profile }) => [profile.remarks, profile.protocol.server.address, name]
        .join(" ").toLocaleLowerCase().includes(needle))
      : allMembers;
    const members = arrangeMembers(matched, view);
    byGroup.delete(groupKey);
    if (needle && !members.length) return;
    const expanded = !!needle || !collapsed.has(groupKey);
    rows.push({
      kind: "group",
      key: groupKey,
      groupKey,
      subscription,
      name,
      members,
      allMembers,
      expanded,
      last: !expanded || !members.length,
    });
    if (expanded)
      members.forEach((item, index) =>
        rows.push({
          kind: "profile",
          key: `profile:${item.profile.id}`,
          groupKey,
          item,
          last: index === members.length - 1,
        }),
      );
  }
  for (const source of [...subscriptions].sort(
    (a, b) => a.sort - b.sort || a.id.localeCompare(b.id),
  ))
    append(`subscription:${source.id}`, source.remarks || unknownName, source);
  // Keep source nodes isolated even before subscription information is available.
  for (const key of byGroup.keys())
    if (key !== LOCAL_GROUP_KEY) append(key, unknownName, null);
  if (byGroup.has(LOCAL_GROUP_KEY)) append(LOCAL_GROUP_KEY, localName, null);
  return rows;
}
