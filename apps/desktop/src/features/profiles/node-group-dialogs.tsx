import { navigateVirtualList } from "./virtual-list-keyboard";
import { useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Button } from "@voya/ui/components/button";
import { Checkbox } from "@voya/ui/components/checkbox";
import { Input } from "@voya/ui/components/input";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@voya/ui/components/dialog";
import type { NodeGroup, NodeGroupAssignment } from "@/ipc/bindings";
import { profileAddress } from "./profile-display";
import type { ServerTableController } from "./use-server-table";

export function NodeGroupDialogs({
  controller,
}: {
  controller: ServerTableController;
}) {
  const { nodeGroups, t, viewportRef } = controller;
  const dialog = nodeGroups.dialog;
  return (
    <Dialog
      open={!!dialog}
      onOpenChange={(open) => {
        if (!open) nodeGroups.close();
      }}
    >
      <DialogContent
        className="max-h-[calc(100dvh-2rem)] overflow-y-auto"
        closeLabel={t("actions.close")}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          nodeGroups.restoreFocus(viewportRef.current);
        }}
      >
        {dialog?.kind === "name" ? (
          <GroupNameForm
            controller={controller}
            group={dialog.group}
            key={dialog.group?.id ?? "new"}
          />
        ) : null}
        {dialog?.kind === "edit" ? (
          <GroupEditForm
            controller={controller}
            group={dialog.group}
            key={dialog.group.id}
          />
        ) : null}
        {dialog?.kind === "delete" ? (
          <>
            <DialogHeader className="shrink-0">
              <DialogTitle>{t("nodeGroups.delete")}</DialogTitle>
              <DialogDescription>
                {t("nodeGroups.deleteDescription", { name: dialog.group.name })}
              </DialogDescription>
            </DialogHeader>
            <GroupError controller={controller} />
            <DialogFooter>
              <Button
                disabled={nodeGroups.busy}
                onClick={nodeGroups.close}
                variant="outline"
              >
                {t("confirm.cancel")}
              </Button>
              <Button
                disabled={nodeGroups.busy}
                onClick={() => void nodeGroups.remove(dialog.group.id)}
                variant="destructive"
              >
                {t("nodeGroups.delete")}
              </Button>
            </DialogFooter>
          </>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}

function GroupError({ controller }: { controller: ServerTableController }) {
  return controller.nodeGroups.error ? (
    <p className="text-sm text-danger" role="alert">
      {controller.nodeGroups.error}
    </p>
  ) : null;
}

function GroupNameForm({
  controller,
  group,
}: {
  controller: ServerTableController;
  group: NodeGroup | null;
}) {
  const { nodeGroups, t } = controller;
  const [name, setName] = useState(group?.name ?? "");
  const duplicate = nodeGroups.snapshot.groups.some(
    (g) => g.id !== group?.id && g.name === name.trim(),
  );
  return (
    <form
      className="flex min-h-0 max-h-[calc(100dvh-2rem)] flex-col"
      onSubmit={(event) => {
        event.preventDefault();
        if (name.trim() && !duplicate)
          void nodeGroups.saveName(group?.id ?? null, name);
      }}
    >
      <DialogHeader className="shrink-0">
        <DialogTitle>{t("nodeGroups.create")}</DialogTitle>
        <DialogDescription>{t("nodeGroups.description")}</DialogDescription>
      </DialogHeader>
      <DialogBody className="grid gap-4">
        <label className="grid gap-2 text-sm">
          {t("nodeGroups.name")}
          <Input
            disabled={nodeGroups.busy}
            value={name}
            onChange={(event) => setName(event.target.value)}
            maxLength={256}
            autoFocus
          />
        </label>
        {duplicate ? (
          <p className="text-sm text-danger" role="alert">
            {t("nodeGroups.duplicateName")}
          </p>
        ) : null}
        <GroupError controller={controller} />
      </DialogBody>
      <DialogFooter className="shrink-0">
        <Button
          disabled={nodeGroups.busy}
          onClick={nodeGroups.close}
          type="button"
          variant="outline"
        >
          {t("confirm.cancel")}
        </Button>
        <Button
          disabled={nodeGroups.busy || !name.trim() || duplicate}
          type="submit"
        >
          {t("panes.profiles.dialog.save")}
        </Button>
      </DialogFooter>
    </form>
  );
}

function GroupEditForm({
  controller,
  group,
}: {
  controller: ServerTableController;
  group: NodeGroup;
}) {
  const { profiles, nodeGroups, t } = controller;
  const [name, setName] = useState(group.name);
  const duplicate = nodeGroups.snapshot.groups.some(
    (g) => g.id !== group.id && g.name === name.trim(),
  );
  const [initial] = useState(
    () =>
      new Set(
        nodeGroups.snapshot.memberships
          .filter((m) => m.groupId === group.id)
          .map((m) => m.profileId),
      ),
  );
  const [selected, setSelected] = useState(() => new Set(initial));
  const [search, setSearch] = useState("");
  const viewport = useRef<HTMLDivElement>(null);
  const members = useMemo(
    () =>
      profiles.filter(
        ({ profile }) =>
          !profile.subscriptionId &&
          `${profile.remarks}\n${profileAddress(profile)}`
            .toLocaleLowerCase()
            .includes(search.trim().toLocaleLowerCase()),
      ),
    [profiles, search],
  );
  const groupNames = new Map(
    nodeGroups.snapshot.groups.map((g) => [g.id, g.name]),
  );
  const assignments = new Map(
    nodeGroups.snapshot.memberships.map((m) => [m.profileId, m.groupId]),
  );
  const virtualizer = useVirtualizer({
    count: members.length,
    getScrollElement: () => viewport.current,
    estimateSize: () => 64,
    getItemKey: (index) => members[index]!.profile.id,
    overscan: 5,
    initialRect: { width: 450, height: 320 },
  });
  const visible = virtualizer.getVirtualItems();
  const rendered = visible.length
    ? visible
    : members
        .slice(0, 10)
        .map((member, index) => ({
          index,
          start: index * 64,
          key: member.profile.id,
        }));
  function toggle(id: string) {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }
  function save() {
    const changes: NodeGroupAssignment[] = [];
    for (const id of new Set([...initial, ...selected]))
      if (initial.has(id) !== selected.has(id))
        changes.push({
          profileId: id,
          groupId: selected.has(id) ? group.id : null,
        });
    if (!name.trim() || duplicate) return;
    if (!changes.length && name.trim() === group.name) nodeGroups.close();
    else void nodeGroups.update(group.id, name, changes);
  }
  return (
    <form
      className="flex min-h-0 max-h-[calc(100dvh-2rem)] flex-col"
      onSubmit={(event) => {
        event.preventDefault();
        save();
      }}
    >
      <DialogHeader className="shrink-0">
        <DialogTitle>
          {t("nodeGroups.editTitle", { name: group.name })}
        </DialogTitle>
        <DialogDescription>
          {t("nodeGroups.membersDescription")}
        </DialogDescription>
      </DialogHeader>
      <DialogBody className="grid gap-4">
        <label className="grid gap-2 text-sm">
          {t("nodeGroups.name")}
          <Input
            disabled={nodeGroups.busy}
            value={name}
            onChange={(event) => setName(event.target.value)}
            maxLength={256}
            autoFocus
          />
        </label>
        {duplicate ? (
          <p className="text-sm text-danger" role="alert">
            {t("nodeGroups.duplicateName")}
          </p>
        ) : null}
        <Input
          disabled={nodeGroups.busy}
          aria-label={t("nodeGroups.searchMembers")}
          placeholder={t("nodeGroups.searchMembers")}
          type="search"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
        <div className="flex flex-wrap items-center gap-2 text-sm">
          <span>{t("nodeGroups.selectedCount", { count: selected.size })}</span>
          <Button
            type="button"
            disabled={nodeGroups.busy}
            size="sm"
            variant="ghost"
            onClick={() =>
              setSelected(
                (previous) =>
                  new Set([...previous, ...members.map((m) => m.profile.id)]),
              )
            }
          >
            {t("nodeGroups.selectResults")}
          </Button>
          <Button
            type="button"
            disabled={nodeGroups.busy}
            size="sm"
            variant="ghost"
            onClick={() =>
              setSelected((previous) => {
                const next = new Set(previous);
                for (const member of members) next.delete(member.profile.id);
                return next;
              })
            }
          >
            {t("nodeGroups.clearResults")}
          </Button>
        </div>
        <div
          className="h-[clamp(8rem,calc(100dvh-26rem),20rem)] overflow-auto rounded border"
          ref={viewport}
          tabIndex={0}
          aria-label={t("nodeGroups.members")}
          onKeyDown={(event) =>
            navigateVirtualList(
              event,
              members.length,
              (index) => virtualizer.scrollToIndex(index),
              "data-member-index",
              '[role="checkbox"]',
            )
          }
        >
          <div
            className="relative"
            style={{ height: virtualizer.getTotalSize() }}
          >
            {rendered.map((row) => {
              const member = members[row.index]!.profile;
              const source =
                groupNames.get(assignments.get(member.id) ?? "") ??
                t("nodeGroups.unassigned");
              return (
                <label
                  className="absolute start-0 top-0 flex h-16 w-full cursor-pointer items-center gap-3 px-3 hover:bg-muted"
                  key={member.id}
                  data-member-index={row.index}
                  style={{ transform: `translateY(${row.start}px)` }}
                >
                  <Checkbox
                    checked={selected.has(member.id)}
                    disabled={nodeGroups.busy}
                    onCheckedChange={() => toggle(member.id)}
                    aria-label={member.remarks || member.id}
                  />
                  <span className="min-w-0">
                    <span className="block truncate" title={member.remarks}>
                      {member.remarks || member.id}
                    </span>
                    <span
                      className="block truncate text-xs text-muted-foreground"
                      title={`${profileAddress(member)} · ${source}`}
                    >
                      {profileAddress(member)} · {source}
                    </span>
                  </span>
                </label>
              );
            })}
          </div>
          {!members.length ? (
            <p className="p-4 text-sm text-muted-foreground">
              {t("nodeGroups.noMatches")}
            </p>
          ) : null}
        </div>
        <GroupError controller={controller} />
      </DialogBody>
      <DialogFooter className="shrink-0">
        <Button
          type="button"
          disabled={nodeGroups.busy}
          onClick={nodeGroups.close}
          variant="outline"
        >
          {t("confirm.cancel")}
        </Button>
        <Button
          type="submit"
          disabled={nodeGroups.busy || !name.trim() || duplicate}
        >
          {t("panes.profiles.dialog.save")}
        </Button>
      </DialogFooter>
    </form>
  );
}
