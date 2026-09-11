import { useRef } from "react";
import { ChevronDown, ChevronRight, Folder, MoreHorizontal } from "lucide-react";
import { Button } from "@voya/ui/components/button";
import { Menubar, MenubarContent, MenubarItem, MenubarMenu, MenubarSeparator, MenubarTrigger } from "@voya/ui/components/menubar";
import { SpeedtestButton } from "./server-table-menus";
import { UNASSIGNED_GROUP_KEY, type NodeListRow } from "./node-list-rows";
import type { ServerTableController } from "./use-server-table";

export function NodeGroupCard({ row, controller }: { row: Extract<NodeListRow, { kind: "group" }>; controller: ServerTableController }) {
  const { nodeGroups, t, handleSpeedtest, handleCancelSpeedtest, speedtestRunning } = controller;
  const group = row.group;
  const trigger = useRef<HTMLButtonElement>(null);
  const opening = useRef(false);
  function open(kind: "members" | "name" | "delete") { if (group) { opening.current = true; nodeGroups.open({ kind, group }, trigger.current); } }
  const position = nodeGroups.snapshot.groups.findIndex((g) => g.id === group?.id);
  return <article className="node-card-surface" data-testid="node-group-card" aria-label={row.name}>
    <Folder aria-hidden="true" className="size-6 shrink-0 text-muted-foreground" />
    <button className="flex min-w-0 flex-1 items-center gap-2 rounded text-start focus-visible:outline-ring" aria-label={row.name} aria-expanded={row.expanded} data-row-focus onClick={() => nodeGroups.toggle(group?.id ?? UNASSIGNED_GROUP_KEY)} type="button">
      {row.expanded ? <ChevronDown aria-hidden="true" className="size-4 shrink-0" /> : <ChevronRight aria-hidden="true" className="size-4 shrink-0" />}
      <span className="min-w-0"><span className="block truncate font-semibold" title={row.name}>{row.name}</span><span className="text-xs text-muted-foreground">{t("nodeGroups.membersCount", { count: row.members.length })}</span></span>
    </button>
    <SpeedtestButton disabled={!row.members.length} label={t("nodeGroups.test")} onCancel={handleCancelSpeedtest} onRun={() => handleSpeedtest({ scope: "profiles", profileIds: row.members.map((p) => p.profile.id) })} running={speedtestRunning} />
    {group ? <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none"><MenubarMenu>
      <MenubarTrigger asChild><Button ref={trigger} aria-label={t("nodeGroups.actions", { name: group.name })} disabled={nodeGroups.busy} size="icon" variant="ghost"><MoreHorizontal aria-hidden="true" className="size-4" /></Button></MenubarTrigger>
      <MenubarContent align="end" onCloseAutoFocus={(event) => { if (opening.current) { event.preventDefault(); opening.current = false; } }}>
        <MenubarItem onSelect={() => open("members")}>{t("nodeGroups.manageMembers")}</MenubarItem>
        <MenubarItem onSelect={() => open("name")}>{t("nodeGroups.rename")}</MenubarItem>
        <MenubarSeparator />
        <MenubarItem disabled={position <= 0} onSelect={() => void nodeGroups.move(group.id, "up")}>{t("panes.profiles.menu.moveUp")}</MenubarItem>
        <MenubarItem disabled={position === nodeGroups.snapshot.groups.length - 1} onSelect={() => void nodeGroups.move(group.id, "down")}>{t("panes.profiles.menu.moveDown")}</MenubarItem>
        <MenubarSeparator />
        <MenubarItem variant="destructive" onSelect={() => open("delete")}>{t("nodeGroups.delete")}</MenubarItem>
      </MenubarContent>
    </MenubarMenu></Menubar> : null}
  </article>;
}
