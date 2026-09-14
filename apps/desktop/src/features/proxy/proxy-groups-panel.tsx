import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Layers, LoaderCircle, Zap } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { cn } from "@voya/ui/lib/utils";
import { getErrorMessage } from "@voya/utils/error";
import {
  listPolicyGroups,
  policyGroupRuntime,
  selectPolicyGroupMember,
  testPolicyGroupDelay,
} from "@/ipc/commands";
import { queryKeys } from "@/ipc/query-keys";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { POLICY_GROUP_STRATEGY_KEYS } from "@/features/profiles/policy-group-labels";
import { profileNameWithoutFlag } from "@/features/profiles/profile-display";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";

/** How often the running group's member and delays are read again while visible. */
const RUNTIME_REFRESH_MS = 3_000;

/**
 * The running policy group as the core sees it: the member traffic goes
 * through right now and each member's last measured delay. Selector members
 * switch live; the automatic strategies only show what the core chose.
 */
export function ProxyGroupsPanel() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const connected = useRuntimeEventStore(
    (state) => state.coreState?.state === "connected",
  );
  const pushToast = useToastStore((state) => state.pushToast);
  const setActiveTab = useShellStore((state) => state.setActiveTab);
  const groupsQuery = useQuery({
    queryFn: listPolicyGroups,
    queryKey: queryKeys.policyGroups,
  });
  const active = groupsQuery.data?.entries.find((entry) => entry.isActive) ?? null;
  const runtimeQuery = useQuery({
    enabled: connected && active !== null,
    queryFn: policyGroupRuntime,
    queryKey: queryKeys.policyGroupRuntime,
    refetchInterval: RUNTIME_REFRESH_MS,
  });
  const [testing, setTesting] = useState(false);

  if (!connected || !active) {
    return (
      <div
        className="flex h-full flex-col items-center justify-center gap-3 p-8 text-center"
        data-testid="proxy-groups-empty"
      >
        <Layers aria-hidden="true" className="size-8 text-muted-foreground" />
        <p className="max-w-sm text-sm text-muted-foreground">
          {t(connected ? "proxy.groups.noActiveGroup" : "proxy.groups.connectToView")}
        </p>
        {connected ? (
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
        )}
      </div>
    );
  }

  const group = active.group;
  const runtime = runtimeQuery.data?.groupId === group.id ? runtimeQuery.data : null;
  const delays = new Map(
    runtime?.members.map((member) => [member.profileId, member.delayMs]) ?? [],
  );

  function reportFailure(error: unknown) {
    pushToast({
      description: getErrorMessage(error),
      severity: "error",
      title: t("proxy.groups.actionFailed"),
    });
  }

  async function choose(profileId: string) {
    // The member switches at once; a failure puts the previous one back.
    const previous = queryClient.getQueryData(queryKeys.policyGroupRuntime);
    if (runtime) {
      queryClient.setQueryData(queryKeys.policyGroupRuntime, { ...runtime, nowProfileId: profileId });
    }
    try {
      await selectPolicyGroupMember(group.id, profileId);
    } catch (error) {
      queryClient.setQueryData(queryKeys.policyGroupRuntime, previous);
      reportFailure(error);
    }
  }

  async function testAll() {
    if (testing) return;
    setTesting(true);
    try {
      const measured = await testPolicyGroupDelay();
      if (measured) queryClient.setQueryData(queryKeys.policyGroupRuntime, measured);
    } catch (error) {
      reportFailure(error);
    } finally {
      setTesting(false);
    }
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
          onClick={() => void testAll()}
          size="sm"
          type="button"
          variant="outline"
        >
          {testing ? (
            <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
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
        {active.members.map((member) => {
          const current = member.profileId === runtime?.nowProfileId;
          const delay = delays.get(member.profileId);
          const name = profileNameWithoutFlag(member.remarks) || member.profileId;
          const content = (
            <>
              <span className="min-w-0 flex-1 truncate text-start">{name}</span>
              <span className="text-xs text-muted-foreground">
                {delay ? t("policyGroups.delay", { ms: delay }) : "—"}
              </span>
            </>
          );
          return group.strategy === "selector" ? (
            <Button
              aria-pressed={current}
              className="justify-between"
              key={member.profileId}
              onClick={() => void choose(member.profileId)}
              type="button"
              variant={current ? "secondary" : "outline"}
            >
              {content}
            </Button>
          ) : (
            <div
              className={cn(
                "flex h-9 items-center gap-2 rounded-md border px-3 text-sm",
                current && "border-primary text-brand",
              )}
              data-current={current || undefined}
              data-testid="proxy-group-member"
              key={member.profileId}
            >
              {content}
            </div>
          );
        })}
      </div>
    </section>
  );
}
