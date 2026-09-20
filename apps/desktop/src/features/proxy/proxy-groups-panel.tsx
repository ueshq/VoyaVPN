import { useQuery } from "@tanstack/react-query";
import { Layers, Zap } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { EmptyState } from "@voya/ui/components/empty-state";
import { Spinner } from "@voya/ui/components/spinner";
import { formatDelay } from "@voya/utils/formatting";
import { listPolicyGroups, selectPolicyGroupMember } from "@/ipc/commands";
import { queryKeys } from "@voya/client/query-keys";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { POLICY_GROUP_STRATEGY_KEYS } from "@voya/features/profiles/policy-group-labels";
import { PolicyGroupMemberChip } from "@/features/profiles/policy-group-member-chip";
import { profileMemberName } from "@voya/features/profiles/profile-display";
import {
  useGroupDelayTest,
  usePolicyGroupMemberSwitch,
  usePolicyGroupRuntime,
} from "@voya/features/profiles/use-policy-group-runtime";
import { useShellStore } from "@/stores/shell-store";
import { toastError } from "@voya/client/toast-store";

/**
 * The running policy group as the core sees it: the member traffic goes
 * through right now and each member's last measured delay. Selector members
 * switch live; the automatic strategies only show what the core chose.
 */
export function ProxyGroupsPanel() {
  const { t } = useI18n();
  const connected = useRuntimeEventStore(
    (state) => state.coreState?.state === "connected",
  );
  const setActiveTab = useShellStore((state) => state.setActiveTab);
  const groupsQuery = useQuery({
    queryFn: listPolicyGroups,
    queryKey: queryKeys.policyGroups,
  });
  const active = groupsQuery.data?.entries.find((entry) => entry.isActive) ?? null;
  const runtime = usePolicyGroupRuntime(active?.group.id ?? null);
  const memberSwitch = usePolicyGroupMemberSwitch();

  /** Run a member action, reporting a failure as a toast. */
  async function runWithToast(operation: () => Promise<unknown>): Promise<boolean> {
    try {
      await operation();
      return true;
    } catch (error) {
      toastError(t("proxy.groups.actionFailed"), error);
      return false;
    }
  }

  const delayTest = useGroupDelayTest(runWithToast);
  const testing = delayTest.testing;
  const delays = new Map(
    runtime?.members.map((member) => [member.profileId, member.delayMs]) ?? [],
  );

  if (!connected || !active) {
    // The same empty state as the live connections tab beside it.
    return (
      <EmptyState
        actions={
          connected ? (
            <Button
              onClick={() => setActiveTab("profiles")}
              size="sm"
              type="button"
              variant="outline"
            >
              {t("proxy.groups.openNodes")}
            </Button>
          ) : (
            <Button onClick={() => setActiveTab("home")} size="sm" type="button" variant="outline">
              {t("activity.goHome")}
            </Button>
          )
        }
        className="h-full content-center"
        data-testid="proxy-groups-empty"
        icon={Layers}
        title={t(connected ? "proxy.groups.noActiveGroup" : "proxy.groups.connectToView")}
      />
    );
  }

  const group = active.group;

  async function choose(profileId: string) {
    // The member switches at once; a failure puts the previous one back.
    await memberSwitch(group.id, profileId, () =>
      runWithToast(() => selectPolicyGroupMember(group.id, profileId)),
    );
  }

  return (
    <section
      aria-label={group.name}
      className="flex h-full min-h-0 flex-col gap-4 overflow-y-auto p-4"
      data-testid="proxy-groups-panel"
    >
      <div className="flex flex-wrap items-center gap-2">
        <Layers aria-hidden="true" className="size-5 text-muted-foreground" />
        <h2 className="truncate font-semibold" title={group.name}>
          {group.name}
        </h2>
        <Badge variant="secondary">{t(POLICY_GROUP_STRATEGY_KEYS[group.strategy])}</Badge>
        <Button
          className="ms-auto"
          disabled={testing}
          onClick={() => void delayTest.test()}
          size="sm"
          type="button"
          variant="outline"
        >
          {testing ? (
            <Spinner className="size-4" />
          ) : (
            <Zap aria-hidden="true" className="size-4" />
          )}
          {t("nodeGroups.test")}
        </Button>
      </div>
      {group.strategy === "selector" ? null : (
        <p className="text-xs text-muted-foreground">{t("proxy.groups.autoMemberHint")}</p>
      )}
      <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
        {active.members.map((member) => (
          <PolicyGroupMemberChip
            current={member.profileId === runtime?.nowProfileId}
            delay={formatDelay(delays.get(member.profileId))}
            delayPlaceholder="—"
            key={member.profileId}
            name={profileMemberName(member.remarks, member.profileId)}
            onChoose={
              group.strategy === "selector"
                ? (profileId) => void choose(profileId)
                : undefined
            }
            profileId={member.profileId}
            stretch
            testId="proxy-group-member"
          />
        ))}
      </div>
    </section>
  );
}
