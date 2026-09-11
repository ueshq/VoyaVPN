import type { NodeGroup, NodeGroupsSnapshot, ProfileListEntry } from "@/ipc/bindings";
import { profileAddress } from "./profile-display";

export const UNASSIGNED_GROUP_KEY = "unassigned";
export type NodeListRow =
  | { kind: "profile"; key: string; item: ProfileListEntry; groupId: string | null }
  | { kind: "group"; key: string; group: NodeGroup | null; name: string; members: ProfileListEntry[]; expanded: boolean };

export function nodeListRows(profiles: ProfileListEntry[], snapshot: NodeGroupsSnapshot, expanded: ReadonlySet<string>, filterText: string, unassignedName: string): NodeListRow[] {
  const filter = filterText.trim().toLocaleLowerCase();
  const assignments = new Map(snapshot.memberships.map((m) => [m.profileId, m.groupId]));
  const byGroup = new Map<string | null, ProfileListEntry[]>([[null, []]]);
  for (const group of snapshot.groups) byGroup.set(group.id, []);
  for (const profile of profiles) {
    const id = assignments.get(profile.profile.id) ?? null;
    (byGroup.get(id) ?? byGroup.get(null))!.push(profile);
  }
  const rows: NodeListRow[] = [];
  for (const group of [...snapshot.groups, null]) {
    const members = byGroup.get(group?.id ?? null)!;
    if (!group && !members.length) continue;
    const name = group?.name ?? unassignedName;
    const groupMatches = name.toLocaleLowerCase().includes(filter);
    const matches = !filter || groupMatches ? members : members.filter(({ profile }) =>
      profile.remarks.toLocaleLowerCase().includes(filter) || profileAddress(profile).toLocaleLowerCase().includes(filter));
    if (filter && !groupMatches && !matches.length) continue;
    const id = group?.id ?? UNASSIGNED_GROUP_KEY;
    const open = !!filter || expanded.has(id);
    rows.push({ kind: "group", key: `group:${id}`, group, name, members, expanded: open });
    if (open) for (const item of matches) rows.push({ kind: "profile", key: `profile:${item.profile.id}`, item, groupId: group?.id ?? null });
  }
  return rows;
}
