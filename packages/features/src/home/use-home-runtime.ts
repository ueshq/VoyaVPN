import { queries } from "@voya/client/queries";
import { useQuery } from "@tanstack/react-query";

import type { TranslationFunction } from "@voya/i18n/core";
import { useI18n } from "@voya/i18n/use-i18n";
import { coreStateOf, runningProfileId, useRuntimeEventStore } from "@voya/client/runtime-event-store";
import type { TunStatus } from "@voya/contracts";
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
  const profilesQuery = useQuery(queries.profileList);

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
  const policyGroupsQuery = useQuery(queries.policyGroups);
  const activeGroup =
    policyGroupsQuery.data?.entries.find((entry) => entry.isActive) ?? null;
  const groupRuntime = usePolicyGroupRuntime(activeGroup?.group.id ?? null);

  const ready = profilesQuery.isSuccess && policyGroupsQuery.isSuccess && (activeProfile !== null || activeGroup !== null);

  function handlePrimaryAction() {
    if (!connected && state !== "cleanupPending" && !ready) return;
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
    ready,
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
    profilesPending: profilesQuery.isPending || policyGroupsQuery.isPending,
    profilesError: profilesQuery.error ?? policyGroupsQuery.error,
    restart,
    retryLastAction,
    retryProfiles: () => { void profilesQuery.refetch(); void policyGroupsQuery.refetch(); },
    runningId,
    state,
    tunProviderSummary,
    tunIssue: homeTunIssue(tun, t),
    // The same line without the provider's own untranslated text, and that
    // text on its own: a view that tucks diagnostics away shows the first and
    // discloses the second.
    tunIssueMessage: homeTunIssue(tun, t, { includeProviderError: false }),
    tunProviderError: tun?.lastProviderError ?? null,
  };
}

/** What each system asks the first time a VPN configuration is added. */
const VPN_PERMISSION_HINT_KEYS = {
  androidVpnService: "home.vpnPermissionHintAndroid",
  iosPacketTunnel: "home.vpnPermissionHintIos",
  macosPacketTunnel: "home.vpnPermissionHint",
} as const satisfies Partial<Record<TunStatus["backend"], string>>;

/** A line under the mode card when the tunnel needs attention, or `null`. */
export function homeTunIssue(
  tun: TunStatus | null,
  t: TranslationFunction,
  options?: { includeProviderError?: boolean },
) {
  if (!tun) return null;
  if (tun.providerPathMismatch) return tunProviderPathMismatchDescription(tun, t);
  // The first connection asks to add a VPN configuration; say what to choose.
  if (tun.providerState === "permissionRequired" && Object.hasOwn(VPN_PERMISSION_HINT_KEYS, tun.backend)) {
    return t(VPN_PERMISSION_HINT_KEYS[tun.backend as keyof typeof VPN_PERMISSION_HINT_KEYS]);
  }
  const needsAttention =
    ["error", "permissionRequired", "missingComponent"].includes(tun.providerState) ||
    tun.lastProviderError;
  return needsAttention ? tunProviderLabel(tun, t, options) : null;
}
