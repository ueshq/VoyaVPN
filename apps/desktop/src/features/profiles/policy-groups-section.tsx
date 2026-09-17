import { Layers, Settings, Trash2, Zap } from "lucide-react";

import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { ConfirmDialog } from "@voya/ui/components/confirm-dialog";
import { Spinner } from "@voya/ui/components/spinner";
import { formatDelay } from "@voya/utils/formatting";
import type { PolicyGroupEntry } from "@/ipc/bindings";

import type { ServerTableController } from "./use-server-table";
import { PolicyGroupDialog } from "./policy-group-dialog";
import { POLICY_GROUP_STRATEGY_KEYS } from "./policy-group-labels";
import { PolicyGroupMemberChip } from "./policy-group-member-chip";
import { profileMemberName } from "./profile-display";
import { SpeedtestButton } from "./server-table-menus";

/**
 * Policy groups above the node list. The editor and the delete confirmation
 * stay mounted without any group, because the toolbar creates the first one.
 */
export function PolicyGroupsSection({ controller }: { controller: ServerTableController }) {
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
      <ConfirmDialog
        cancelLabel={t("confirm.cancel")}
        confirmLabel={t("actions.delete")}
        description={
          <>
            {t("policyGroups.deleteHint")}
            {/* Same warning as deleting the node in use: say it before, not after. */}
            {deletingPolicyGroup?.isActive && controller.coreConnected ? (
              <span className="mt-2 block font-medium text-warning">
                {t("policyGroups.deleteActiveHint")}
              </span>
            ) : null}
          </>
        }
        destructive
        error={deletingPolicyGroup ? operationError : null}
        onConfirm={(event) => {
          event.preventDefault();
          void removePolicyGroup();
        }}
        onOpenChange={(open) => !open && setDeletingPolicyGroup(null)}
        open={deletingPolicyGroup !== null}
        title={t("policyGroups.deleteTitle", { name: deletingPolicyGroup?.group.name })}
      />
    </>
  );
}

function PolicyGroupCard({
  controller,
  entry,
}: {
  controller: ServerTableController;
  entry: PolicyGroupEntry;
}) {
  const {
    activatePolicyGroup,
    choosePolicyGroupMember,
    coreConnected,
    handleCancelSpeedtest,
    handleSpeedtest,
    openGroupEditor,
    policyGroupRuntimeState,
    policyGroupSubscriptions,
    profiles,
    setDeletingPolicyGroup,
    speedtestProgress,
    speedtestRunning,
    speedtestSource,
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
  // A running group reports its members' delays; otherwise the last speed test does.
  const delays = new Map(
    live
      ? live.members.map((member) => [member.profileId, member.delayMs])
      : members.map((member) => [
          member.profileId,
          profiles.find((item) => item.profile.id === member.profileId)?.metrics.delayMs || null,
        ]),
  );
  const speedtestKey = `policy:${group.id}`;
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
              title={t("nodeGroups.test")}
              type="button"
              variant="ghost"
            >
              {testingPolicyGroup ? (
                <Spinner className="size-4" />
              ) : (
                <Zap aria-hidden="true" className="size-4" />
              )}
              <span data-slot="button-label">{t("nodeGroups.test")}</span>
            </Button>
          ) : (
            // Before the group runs, its test measures the member nodes directly.
            <SpeedtestButton
              busyElsewhere={speedtestRunning && speedtestSource !== speedtestKey}
              disabled={members.length === 0}
              label={t("nodeGroups.test")}
              onCancel={handleCancelSpeedtest}
              onRun={() =>
                handleSpeedtest(
                  { profileIds: members.map((member) => member.profileId), scope: "profiles" },
                  speedtestKey,
                )
              }
              progress={speedtestProgress}
              running={speedtestRunning && speedtestSource === speedtestKey}
              variant="ghost"
            />
          )}
          {/* The same outline "use" action as a node row. */}
          <span title={members.length === 0 ? t("nodeGroups.empty") : undefined}>
          <Button
            disabled={inUse || switchingPolicyGroupId !== null || members.length === 0}
            onClick={() => void activatePolicyGroup(group.id)}
            size="sm"
            type="button"
            variant={inUse ? "secondary" : "outline"}
          >
            {switchingPolicyGroupId === group.id ? (
              <Spinner className="size-4" />
            ) : null}
            {t(inUse ? "policyGroups.inUse" : "policyGroups.use")}
          </Button>
          </span>
          <Button
            aria-label={t("policyGroups.editNamed", { name: group.name })}
            onClick={() => openGroupEditor(group)}
            size="icon-sm"
            title={t("policyGroups.editNamed", { name: group.name })}
            type="button"
            variant="ghost"
          >
            <Settings aria-hidden="true" className="size-4" />
          </Button>
          <Button
            aria-label={t("policyGroups.deleteNamed", { name: group.name })}
            onClick={() => setDeletingPolicyGroup(entry)}
            size="icon-sm"
            title={t("policyGroups.deleteNamed", { name: group.name })}
            type="button"
            variant="ghost"
          >
            <Trash2 aria-hidden="true" className="size-4" />
          </Button>
        </div>
      </div>
      {members.length ? (
        <div className="flex flex-wrap gap-2 px-5 pb-4">
          {members.map((member) => (
            <PolicyGroupMemberChip
              current={member.profileId === currentId}
              delay={formatDelay(delays.get(member.profileId))}
              key={member.profileId}
              name={profileMemberName(member.remarks, member.profileId)}
              onChoose={
                group.strategy === "selector"
                  ? (profileId) => void choosePolicyGroupMember(group.id, profileId)
                  : undefined
              }
              profileId={member.profileId}
            />
          ))}
        </div>
      ) : (
        <p className="node-group-empty">{t("nodeGroups.empty")}</p>
      )}
    </article>
  );
}
