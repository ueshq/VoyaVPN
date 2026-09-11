import type {
  NodeGroup,
  NodeGroupsSnapshot,
  ProfileListEntry,
  Subscription,
} from "@/ipc/bindings";

export const UNASSIGNED_GROUP_KEY = "unassigned";
type GroupBoundary = { groupKey: string; last: boolean };
export type NodeListRow = GroupBoundary &
  (
    | { kind: "profile"; key: string; item: ProfileListEntry }
    | {
        kind: "group";
        key: string;
        group: NodeGroup | null;
        subscription: Subscription | null;
        name: string;
        members: ProfileListEntry[];
        expanded: boolean;
      }
  );

/** Manual membership never overrides subscription provenance. */
export function profilesByNodeGroup(
  profiles: ProfileListEntry[],
  snapshot: NodeGroupsSnapshot,
) {
  const assignments = new Map(
    snapshot.memberships.map((m) => [m.profileId, m.groupId]),
  );
  const byGroup = new Map<string | null, ProfileListEntry[]>([[null, []]]);
  for (const group of snapshot.groups) byGroup.set(group.id, []);
  for (const profile of profiles) {
    if (profile.profile.subscriptionId) continue;
    const id = assignments.get(profile.profile.id) ?? null;
    (byGroup.get(id) ?? byGroup.get(null))!.push(profile);
  }
  return byGroup;
}

export function nodeListRows(
  profiles: ProfileListEntry[],
  snapshot: NodeGroupsSnapshot,
  collapsed: ReadonlySet<string>,
  unassignedName: string,
  subscriptions: readonly Subscription[] = [],
  unknownName = unassignedName,
): NodeListRow[] {
  const byGroup = profilesByNodeGroup(profiles, snapshot);
  const rows: NodeListRow[] = [];
  function append(
    groupKey: string,
    name: string,
    members: ProfileListEntry[],
    group: NodeGroup | null,
    subscription: Subscription | null,
  ) {
    const expanded = !collapsed.has(groupKey);
    rows.push({
      kind: "group",
      key: groupKey,
      groupKey,
      group,
      subscription,
      name,
      members,
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
  const sources = new Map<string, ProfileListEntry[]>();
  for (const profile of profiles)
    if (profile.profile.subscriptionId) {
      const id = profile.profile.subscriptionId;
      const members = sources.get(id) ?? [];
      members.push(profile);
      sources.set(id, members);
    }
  for (const source of [...subscriptions].sort(
    (a, b) => a.sort - b.sort || a.id.localeCompare(b.id),
  )) {
    append(
      `subscription:${source.id}`,
      source.remarks,
      sources.get(source.id) ?? [],
      null,
      source,
    );
    sources.delete(source.id);
  }
  // Retain read-only source groups when subscription metadata cannot be loaded.
  for (const [id, members] of sources)
    append(`subscription:${id}`, unknownName, members, null, null);
  for (const group of snapshot.groups)
    append(
      `manual:${group.id}`,
      group.name,
      byGroup.get(group.id)!,
      group,
      null,
    );
  const unassigned = byGroup.get(null)!;
  if (unassigned.length)
    append(UNASSIGNED_GROUP_KEY, unassignedName, unassigned, null, null);
  return rows;
}
