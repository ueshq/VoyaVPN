import type { NodeGroup, NodeGroupsSnapshot, ProfileListEntry } from "@/ipc/bindings";

export const UNASSIGNED_GROUP_KEY = "unassigned";
type GroupBoundary = { groupKey: string; last: boolean };
export type NodeListRow = GroupBoundary & (
  | { kind: "profile"; key: string; item: ProfileListEntry }
  | { kind: "group"; key: string; group: NodeGroup | null; name: string; members: ProfileListEntry[]; expanded: boolean }
);

/** Shared by the list and export: membership follows node IDs, never viewport rows. */
export function profilesByNodeGroup(profiles: ProfileListEntry[], snapshot: NodeGroupsSnapshot) {
  const assignments = new Map(snapshot.memberships.map((m) => [m.profileId, m.groupId]));
  const byGroup = new Map<string | null, ProfileListEntry[]>([[null, []]]);
  for (const group of snapshot.groups) byGroup.set(group.id, []);
  for (const profile of profiles) {
    const id = assignments.get(profile.profile.id) ?? null;
    (byGroup.get(id) ?? byGroup.get(null))!.push(profile);
  }
  return byGroup;
}

export function nodeListRows(profiles: ProfileListEntry[], snapshot: NodeGroupsSnapshot, collapsed: ReadonlySet<string>, unassignedName: string): NodeListRow[] {
  const byGroup = profilesByNodeGroup(profiles, snapshot);
  const rows: NodeListRow[] = [];
  for (const group of [...snapshot.groups, null]) {
    const members = byGroup.get(group?.id ?? null)!;
    if (!group && !members.length) continue;
    const name = group?.name ?? unassignedName;
    const groupKey = group?.id ?? UNASSIGNED_GROUP_KEY;
    const expanded = !collapsed.has(groupKey);
    rows.push({ kind: "group", key: `group:${groupKey}`, groupKey, group, name, members, expanded, last: !expanded || !members.length });
    if (expanded) members.forEach((item, index) => rows.push({ kind: "profile", key: `profile:${item.profile.id}`, groupKey, item, last: index === members.length - 1 }));
  }
  return rows;
}
