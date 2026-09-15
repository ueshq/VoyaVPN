import { useQuery } from "@tanstack/react-query";

import type { TranslationFunction } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { listPolicyGroups, listProfiles, policyGroupRuntime } from "@/ipc/commands";
import { coreStateOf, runningProfileId, useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { TunStatus } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { runtimeActionPending, useRuntimeActionStore } from "@/stores/runtime-action-store";

import { isRuntimeTransitioning, runRuntimeAction } from "./runtime-action";
import { tunProviderLabel, tunProviderPathMismatchDescription } from "./tun-provider-text";

/**
 * Runtime controller for the Home screen: connect/disconnect/restart with
 * elevation + missing-core handling, node selection/switching, and the seeded
 * TUN live state. How traffic is captured is chosen in Settings, never here.
 */
/** How often the running group's current member is read again, as on the Nodes page. */
const GROUP_RUNTIME_REFRESH_MS = 3_000;

export function useHomeRuntime() {
  const { t } = useI18n();
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const tun = useRuntimeEventStore((state) => state.tun);
  const pending = useRuntimeActionStore(runtimeActionPending);
  const modePending = useRuntimeActionStore((state) => state.modePending);
  const lastError = useRuntimeActionStore((state) => state.lastError);
  // Shares the ProfilesScreen query cache (same key) so resolving the active
  // node's name here costs no extra fetch and stays in sync after a switch.
  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, null),
    queryKey: queryKeys.profileList,
  });

  const state = coreStateOf(coreState);
  const connected = state === "connected";
  const inProgress = isRuntimeTransitioning(state);
  const busy = inProgress || pending;

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
    queryFn: listPolicyGroups,
    queryKey: queryKeys.policyGroups,
  });
  const activeGroup =
    policyGroupsQuery.data?.entries.find((entry) => entry.isActive) ?? null;
  const groupRuntimeQuery = useQuery({
    enabled: connected && activeGroup !== null,
    queryFn: policyGroupRuntime,
    queryKey: queryKeys.policyGroupRuntime,
    refetchInterval: GROUP_RUNTIME_REFRESH_MS,
  });
  const groupRuntime =
    connected && activeGroup && groupRuntimeQuery.data?.groupId === activeGroup.group.id
      ? groupRuntimeQuery.data
      : null;

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

  return {
    activeGroup,
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
