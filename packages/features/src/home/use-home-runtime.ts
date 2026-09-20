import { useQuery } from "@tanstack/react-query";

import type { TranslationFunction } from "@voya/i18n/core";
import { useI18n } from "@voya/i18n/use-i18n";
import { voyaCommands } from "@voya/client/transport";
import { coreStateOf, runningProfileId, useRuntimeEventStore } from "@voya/client/runtime-event-store";
import type { TunStatus } from "@voya/contracts";
import { queryKeys } from "@voya/client/query-keys";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { usePolicyGroupRuntime } from "../profiles/use-policy-group-runtime";

import {
  isRuntimeTransitioning,
  runRuntimeAction,
  useRuntimeBusy,
} from "@voya/client/runtime-action";
import { tunProviderLabel, tunProviderPathMismatchDescription } from "../shell/tun-provider-text";

/**
 * Runtime controller for the Home screen: connect/disconnect/restart with
 * elevation + missing-core handling, node selection/switching, and the seeded
 * TUN live state. How traffic is captured is chosen in Settings, never here.
 */
export function useHomeRuntime() {
  const { t } = useI18n();
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const tun = useRuntimeEventStore((state) => state.tun);
  const modePending = useRuntimeActionStore((state) => state.modePending);
  const lastError = useRuntimeActionStore((state) => state.lastError);
  // Shares the ProfilesScreen query cache (same key) so resolving the active
  // node's name here costs no extra fetch and stays in sync after a switch.
  const profilesQuery = useQuery({
    queryFn: () => voyaCommands().listProfileSummaries(),
    queryKey: queryKeys.profileList,
  });

  const state = coreStateOf(coreState);
  const connected = state === "connected";
  // Home separates "a runtime action runs" (busy) from "the core itself is
  // mid-transition" (inProgress): the button label and the mode-pending hint
  // depend on the difference.
  const busy = useRuntimeBusy();
  const inProgress = isRuntimeTransitioning(state);

  const activeProfile =
    profilesQuery.data?.entries.find((item) => item.isActive) ?? null;
  // Connection details follow the running node rather than the saved selection.
  const runningId = runningProfileId(coreState);
  const tunEnabled = tun?.enabled ?? false;
  const tunProviderSummary = tun ? tunProviderLabel(tun, t) : null;

  const runningEntry = runningId
    ? (profilesQuery.data?.entries.find(
        (item) => item.profile.id === runningId,
      ) ?? null)
    : null;
  const nodeEntry = connected ? runningEntry : activeProfile;
  // Shares the node page's query, so activating a group there shows up here.
  const policyGroupsQuery = useQuery({
    queryFn: () => voyaCommands().listPolicyGroups(),
    queryKey: queryKeys.policyGroups,
  });
  const activeGroup =
    policyGroupsQuery.data?.entries.find((entry) => entry.isActive) ?? null;
  const groupRuntime = usePolicyGroupRuntime(activeGroup?.group.id ?? null);

  function handlePrimaryAction() {
    const action = connected || state === "cleanupPending" ? "disconnect" : "connect";
    void runRuntimeAction(action, t, { inline: true });
  }

  function restart() {
    void runRuntimeAction("restart", t, { inline: true });
  }

  function retryLastAction() {
    const failed = useRuntimeActionStore.getState().lastError;
    if (failed) void runRuntimeAction(failed.action, t, { inline: true });
  }

  /**
   * Whether there is anything to run at all.
   *
   * A pending or failed read is not an empty list: only a settled, empty one
   * is, and a running core proves there was a node whatever the read says.
   * Both shells key their empty state and their map marker off this.
   */
  const hasNodes =
    connected ||
    state === "cleanupPending" ||
    profilesQuery.isPending ||
    profilesQuery.error !== null ||
    (profilesQuery.data?.entries.length ?? 0) > 0;

  return {
    activeGroup,
    hasNodes,
    groupRuntime,
    nodeEntry,
    busy,
    connected,
    tunEnabled,
    handlePrimaryAction,
    inProgress,
    lastError,
    mainPid: coreState?.mainPid ?? null,
    modePending,
    profiles: profilesQuery.data?.entries ?? [],
    profilesPending: profilesQuery.isPending,
    profilesError: profilesQuery.error,
    restart,
    retryLastAction,
    retryProfiles: () => void profilesQuery.refetch(),
    runningId,
    state,
    tunProviderSummary,
    tunIssue: homeTunIssue(tun, tunProviderSummary, t),
  };
}

/** A line under the mode card when the tunnel needs attention, or `null`. */
function homeTunIssue(
  tun: TunStatus | null,
  summary: string | null,
  t: TranslationFunction,
) {
  if (!tun) return null;
  if (tun.providerPathMismatch) return tunProviderPathMismatchDescription(tun, t);
  // The first connection on macOS asks to add a VPN configuration.
  if (tun.backend === "macosPacketTunnel" && tun.providerState === "permissionRequired") {
    return t("home.vpnPermissionHint");
  }
  const needsAttention =
    ["error", "permissionRequired", "missingComponent"].includes(tun.providerState) ||
    tun.lastProviderError;
  return needsAttention ? summary : null;
}
