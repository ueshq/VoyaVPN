import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import type { TranslationFunction } from "@voya/i18n";
import type {
  PolicyGroup,
  PolicyGroupEntry,
  PolicyGroupListing,
  PolicyGroupRuntime,
  ProfileListing,
} from "@/ipc/bindings";
import {
  deletePolicyGroups,
  listPolicyGroups,
  listSubscriptions,
  policyGroupRuntime,
  selectPolicyGroupMember,
  setActivePolicyGroup,
  testPolicyGroupDelay,
} from "@/ipc/commands";
import { profilesQueryKey, queryKeys } from "@/ipc/query-keys";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { refreshRuntimeStatusAndReport } from "@/ipc/runtime-status";
import { runtimeActionPending, useRuntimeActionStore } from "@/stores/runtime-action-store";
import { useToastStore } from "@/stores/toast-store";
import {
  executeRuntimeAction,
  isRuntimeTransitioning,
  reportRuntimeActionError,
} from "@/features/home/runtime-action";

import { profileNameWithoutFlag } from "./profile-display";
import type { NodeOperation } from "./use-node-operation";

/** How often a running group's live member and delays are read again; the same everywhere a group shows. */
const RUNTIME_REFRESH_MS = 3_000;
/** Marks a group switch in the shared runtime-action guard, apart from node ids. */
const GROUP_SWITCH_PREFIX = "group:";

export function usePolicyGroups(
  operation: Pick<NodeOperation, "runOperation">,
  t: TranslationFunction,
) {
  const queryClient = useQueryClient();
  const coreConnected = useRuntimeEventStore(
    (state) => state.coreState?.state === "connected",
  );
  const switchingId = useRuntimeActionStore((state) => state.switchingId);
  const policyGroupsQuery = useQuery({
    queryFn: listPolicyGroups,
    queryKey: queryKeys.policyGroups,
  });
  const policyGroupEntries = policyGroupsQuery.data?.entries ?? [];
  const runtimeQuery = useQuery({
    enabled: coreConnected && policyGroupEntries.some((entry) => entry.isActive),
    queryFn: policyGroupRuntime,
    queryKey: queryKeys.policyGroupRuntime,
    refetchInterval: RUNTIME_REFRESH_MS,
  });
  const subscriptionsQuery = useQuery({
    queryFn: listSubscriptions,
    queryKey: queryKeys.subscriptions,
  });
  const [editingPolicyGroup, setEditingPolicyGroup] = useState<PolicyGroup | null>(null);
  const [policyGroupEditorOpen, setPolicyGroupEditorOpen] = useState(false);
  const [deletingPolicyGroup, setDeletingPolicyGroup] = useState<PolicyGroupEntry | null>(null);
  const [testingPolicyGroup, setTestingPolicyGroup] = useState(false);

  function openGroupEditor(group: PolicyGroup | null) {
    setEditingPolicyGroup(group);
    setPolicyGroupEditorOpen(true);
  }

  /** The same two steps as choosing a node: make it active, then connect or restart. */
  async function activatePolicyGroup(id: string) {
    const currentState = useRuntimeEventStore.getState().coreState?.state ?? "disconnected";
    if (
      runtimeActionPending() ||
      isRuntimeTransitioning(currentState) ||
      currentState === "cleanupPending"
    ) {
      return false;
    }
    useRuntimeActionStore.setState({ switchingId: `${GROUP_SWITCH_PREFIX}${id}` });
    const action = currentState === "connected" ? "restart" : "connect";
    try {
      // A group replaces a node used on its own; say which one it set aside.
      const replacedNode = policyGroupEntries.some((entry) => entry.isActive)
        ? null
        : queryClient
            .getQueryData<ProfileListing>(profilesQueryKey(""))
            ?.entries.find((entry) => entry.isActive)?.profile;
      await setActivePolicyGroup(id);
      if (replacedNode) {
        useToastStore.getState().pushToast({
          description: t("policyGroups.replacedNode", {
            node: profileNameWithoutFlag(replacedNode.remarks) || replacedNode.id,
          }),
          severity: "info",
          title: t("policyGroups.switchedTitle"),
        });
      }
      const status = await executeRuntimeAction(action);
      return status.state === "connected";
    } catch (error) {
      reportRuntimeActionError(error, action, t);
      return false;
    } finally {
      try {
        await refreshRuntimeStatusAndReport(t);
      } finally {
        useRuntimeActionStore.setState({ switchingId: null });
      }
    }
  }

  /** The chip moves at once; the next runtime read confirms it and a failure puts it back. */
  async function choosePolicyGroupMember(groupId: string, profileId: string) {
    const previousRuntime = queryClient.getQueryData<PolicyGroupRuntime | null>(
      queryKeys.policyGroupRuntime,
    );
    const previousGroups = queryClient.getQueryData<PolicyGroupListing>(queryKeys.policyGroups);
    if (previousRuntime?.groupId === groupId) {
      queryClient.setQueryData(queryKeys.policyGroupRuntime, {
        ...previousRuntime,
        nowProfileId: profileId,
      });
    }
    if (previousGroups) {
      queryClient.setQueryData<PolicyGroupListing>(queryKeys.policyGroups, {
        ...previousGroups,
        entries: previousGroups.entries.map((entry) =>
          entry.group.id === groupId
            ? { ...entry, group: { ...entry.group, selectedProfileId: profileId } }
            : entry,
        ),
      });
    }
    const saved = await operation.runOperation(() => selectPolicyGroupMember(groupId, profileId));
    if (!saved) {
      queryClient.setQueryData(queryKeys.policyGroupRuntime, previousRuntime);
      queryClient.setQueryData(queryKeys.policyGroups, previousGroups);
    }
    return saved;
  }

  async function testRunningPolicyGroup() {
    if (testingPolicyGroup) return;
    setTestingPolicyGroup(true);
    try {
      await operation.runOperation(async () => {
        const runtime = await testPolicyGroupDelay();
        if (runtime) queryClient.setQueryData(queryKeys.policyGroupRuntime, runtime);
      });
    } finally {
      setTestingPolicyGroup(false);
    }
  }

  async function removePolicyGroup() {
    const entry = deletingPolicyGroup;
    if (!entry) return;
    const removed = await operation.runOperation(() => deletePolicyGroups([entry.group.id]));
    if (removed) setDeletingPolicyGroup(null);
  }

  return {
    activatePolicyGroup,
    choosePolicyGroupMember,
    coreConnected,
    deletingPolicyGroup,
    editingPolicyGroup,
    openGroupEditor,
    policyGroupEditorOpen,
    policyGroupEntries,
    // Only a connected core has a running group; stale data never outlives it.
    policyGroupRuntimeState: coreConnected ? (runtimeQuery.data ?? null) : null,
    policyGroupSubscriptions: subscriptionsQuery.data ?? [],
    removePolicyGroup,
    setDeletingPolicyGroup,
    setPolicyGroupEditorOpen,
    switchingPolicyGroupId: switchingId?.startsWith(GROUP_SWITCH_PREFIX)
      ? switchingId.slice(GROUP_SWITCH_PREFIX.length)
      : null,
    testRunningPolicyGroup,
    testingPolicyGroup,
  };
}
