import { Layers, LoaderCircle, Power, Settings, Trash2, Zap } from "lucide-react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@voya/ui/components/alert-dialog";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { buttonVariants } from "@voya/ui/components/button-variants";
import { cn } from "@voya/ui/lib/utils";
import type { PolicyGroupEntry } from "@/ipc/bindings";

import type { PolicyGroupsController } from "./node-controller-types";
import { PolicyGroupDialog } from "./policy-group-dialog";
import { POLICY_GROUP_STRATEGY_KEYS } from "./policy-group-labels";
import { profileNameWithoutFlag } from "./profile-display";

/**
 * Policy groups above the node list. The editor and the delete confirmation
 * stay mounted without any group, because the toolbar creates the first one.
 */
export function PolicyGroupsSection({ controller }: { controller: PolicyGroupsController }) {
  const {
    deletingPolicyGroup,
    operationError,
    policyGroupEntries,
    removePolicyGroup,
    setDeletingPolicyGroup,
    t,
  } = controller;

  return (
    <>
      {policyGroupEntries.length ? (
        <section
          aria-label={t("policyGroups.title")}
          className="grid max-h-[45%] shrink-0 gap-2 overflow-y-auto"
          data-testid="policy-groups-section"
        >
          {policyGroupEntries.map((entry) => (
            <PolicyGroupCard controller={controller} entry={entry} key={entry.group.id} />
          ))}
        </section>
      ) : null}
      <PolicyGroupDialog
        group={controller.editingPolicyGroup}
        nodes={controller.profiles}
        onOpenChange={controller.setPolicyGroupEditorOpen}
        open={controller.policyGroupEditorOpen}
        subscriptions={controller.policyGroupSubscriptions}
      />
      <AlertDialog
        open={deletingPolicyGroup !== null}
        onOpenChange={(open) => !open && setDeletingPolicyGroup(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("policyGroups.deleteTitle", { name: deletingPolicyGroup?.group.name })}
            </AlertDialogTitle>
            <AlertDialogDescription>{t("policyGroups.deleteHint")}</AlertDialogDescription>
          </AlertDialogHeader>
          {operationError && deletingPolicyGroup ? (
            <p className="text-sm text-danger" role="alert">
              {operationError}
            </p>
          ) : null}
          <AlertDialogFooter>
            <AlertDialogCancel>{t("confirm.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              className={buttonVariants({ variant: "destructive" })}
              onClick={(event) => {
                event.preventDefault();
                void removePolicyGroup();
              }}
            >
              {t("actions.delete")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

function PolicyGroupCard({
  controller,
  entry,
}: {
  controller: PolicyGroupsController;
  entry: PolicyGroupEntry;
}) {
  const {
    activatePolicyGroup,
    choosePolicyGroupMember,
    coreConnected,
    openGroupEditor,
    policyGroupRuntimeState,
    policyGroupSubscriptions,
    setDeletingPolicyGroup,
    switchingPolicyGroupId,
    t,
    testRunningPolicyGroup,
    testingPolicyGroup,
  } = controller;
  const { group, isActive, members } = entry;
  const live = policyGroupRuntimeState?.groupId === group.id ? policyGroupRuntimeState : null;
  // Without a running core a selector still shows the member it starts on.
  const currentId =
    live?.nowProfileId ??
    (group.strategy === "selector"
      ? (group.selectedProfileId ?? members[0]?.profileId ?? null)
      : null);
  const delays = new Map(live?.members.map((member) => [member.profileId, member.delayMs]) ?? []);
  const inUse = isActive && coreConnected;
  const source = group.autoCreated
    ? policyGroupSubscriptions.find((item) => item.id === group.sourceSubscriptionId)
    : undefined;

  return (
    <article
      aria-label={group.name}
      className="node-group-surface rounded-xl bg-card"
      data-active={isActive || undefined}
      data-testid="policy-group-card"
    >
      <div className="node-group-heading">
        <div className="flex min-w-0 flex-1 items-center gap-3">
          <Layers aria-hidden="true" className="size-5 shrink-0 text-muted-foreground" />
          <div className="min-w-0">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <span className="truncate font-semibold" title={group.name}>
                {group.name}
              </span>
              <Badge variant="secondary">{t(POLICY_GROUP_STRATEGY_KEYS[group.strategy])}</Badge>
              {group.autoCreated ? (
                <Badge variant="outline">
                  {source?.remarks
                    ? t("policyGroups.autoCreatedFrom", { name: source.remarks })
                    : t("policyGroups.autoCreated")}
                </Badge>
              ) : null}
              {/* Once connected the button says "In use" instead. */}
              {isActive && !inUse ? <Badge>{t("policyGroups.active")}</Badge> : null}
            </div>
            <span className="text-xs text-muted-foreground">
              {t("nodeGroups.membersCount", { count: members.length })}
            </span>
          </div>
        </div>
        <div className="node-group-actions">
          {live ? (
            <Button
              disabled={testingPolicyGroup}
              onClick={() => void testRunningPolicyGroup()}
              size="sm"
              type="button"
              variant="outline"
            >
              {testingPolicyGroup ? (
                <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
              ) : (
                <Zap aria-hidden="true" className="size-4" />
              )}
              {t("nodeGroups.test")}
            </Button>
          ) : null}
          <span title={members.length === 0 ? t("nodeGroups.empty") : undefined}>
          <Button
            disabled={inUse || switchingPolicyGroupId !== null || members.length === 0}
            onClick={() => void activatePolicyGroup(group.id)}
            size="sm"
            type="button"
            variant={inUse ? "secondary" : "default"}
          >
            {switchingPolicyGroupId === group.id ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <Power aria-hidden="true" className="size-4" />
            )}
            {t(inUse ? "policyGroups.inUse" : "policyGroups.use")}
          </Button>
          </span>
          <Button
            aria-label={t("policyGroups.editNamed", { name: group.name })}
            onClick={() => openGroupEditor(group)}
            size="icon"
            type="button"
            variant="ghost"
          >
            <Settings aria-hidden="true" className="size-4" />
          </Button>
          <Button
            aria-label={t("policyGroups.deleteNamed", { name: group.name })}
            onClick={() => setDeletingPolicyGroup(entry)}
            size="icon"
            type="button"
            variant="ghost"
          >
            <Trash2 aria-hidden="true" className="size-4" />
          </Button>
        </div>
      </div>
      {members.length ? (
        <div className="flex flex-wrap gap-2 px-5 pb-4">
          {members.map((member) => {
            const current = member.profileId === currentId;
            const delay = delays.get(member.profileId);
            const name = profileNameWithoutFlag(member.remarks) || member.profileId;
            const detail = delay ? (
              <span className="text-xs text-muted-foreground">
                {t("policyGroups.delay", { ms: delay })}
              </span>
            ) : null;
            return group.strategy === "selector" ? (
              <Button
                aria-pressed={current}
                key={member.profileId}
                onClick={() => void choosePolicyGroupMember(group.id, member.profileId)}
                size="sm"
                type="button"
                variant={current ? "secondary" : "outline"}
              >
                <span className="max-w-48 truncate">{name}</span>
                {detail}
              </Button>
            ) : (
              <span
                className={cn(
                  "inline-flex h-8 items-center gap-2 rounded-md border px-3 text-sm",
                  current && "border-primary text-brand",
                )}
                data-current={current || undefined}
                key={member.profileId}
              >
                <span className="max-w-48 truncate">{name}</span>
                {detail}
              </span>
            );
          })}
        </div>
      ) : (
        <p className="node-group-empty">{t("nodeGroups.empty")}</p>
      )}
    </article>
  );
}
