import { queries, refreshQueries } from "@voya/client/queries";
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import type { TranslationFunction } from "@voya/i18n/core";
import type { PolicyGroup, PolicyGroupEntry, ProfileSummaryListing } from "@voya/contracts";
import { voyaCommands } from "@voya/client/transport";
import { queryKeys } from "@voya/client/query-keys";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useToastStore } from "@voya/client/toast-store";
import { activateSelection, chooseSelection } from "@voya/client/runtime-action";

import { useBusyAction } from "../forms/use-busy-action";
import { profileMemberName } from "./profile-display";
import { useActivePolicyGroup, useGroupDelayTest, usePolicyGroupMemberSwitch } from "./use-policy-group-runtime";
import type { NodeOperation } from "./use-node-operation";

/** Marks a group switch in the shared runtime-action guard, apart from node ids. */
const GROUP_SWITCH_PREFIX = "group:";

export function usePolicyGroups(
  operation: Pick<NodeOperation, "runOperation" | "setOperationError">,
  t: TranslationFunction,
  /** See [`useActivePolicyGroup`]: whether the running group's state is read. */
  { live = true, inline = false }: { live?: boolean; inline?: boolean } = {},
) {
  const queryClient = useQueryClient();
  const coreConnected = useRuntimeEventStore((state) => state.coreState?.state === "connected");
  const switchingId = useRuntimeActionStore((state) => state.switchingId);
  const { policyGroupsQuery, runtime: policyGroupRuntimeState } = useActivePolicyGroup({ live });
  const policyGroupEntries = policyGroupsQuery.data?.entries ?? [];
  const memberSwitch = usePolicyGroupMemberSwitch();
  const delayTest = useGroupDelayTest(operation.runOperation);
  const subscriptionsQuery = useQuery(queries.subscriptions);
  const [editingPolicyGroup, setEditingPolicyGroup] = useState<PolicyGroup | null>(null);
  const [policyGroupEditorOpen, setPolicyGroupEditorOpen] = useState(false);
  const [deletingPolicyGroup, setDeletingGroup] = useState<PolicyGroupEntry | null>(null);
  const { busy: deletingPolicyGroupPending, run: runDelete } = useBusyAction();

  // The confirmation shows the page's operation error as its own. Opening it
  // starts clean, or a failed export from a minute ago would read as the
  // reason this group cannot be deleted.
  function setDeletingPolicyGroup(entry: PolicyGroupEntry | null) {
    if (entry) operation.setOperationError(null);
    setDeletingGroup(entry);
  }

  function openGroupEditor(group: PolicyGroup | null) {
    setEditingPolicyGroup(group);
    setPolicyGroupEditorOpen(true);
  }

  /** Makes the group active, then connects, or restarts a running core, with it. */
  function activatePolicyGroup(id: string) {
    return activateSelection(
      `${GROUP_SWITCH_PREFIX}${id}`,
      t,
      async () => {
        // A group replaces a node used on its own; say which one it set aside.
        const replacedNode = policyGroupEntries.some((entry) => entry.isActive)
          ? null
          : queryClient
              .getQueryData<ProfileSummaryListing>(queryKeys.profileList)
              ?.entries.find((entry) => entry.isActive)?.profile;
        await voyaCommands().setActivePolicyGroup(id);
        if (replacedNode) {
          useToastStore.getState().pushToast({
            description: t("policyGroups.replacedNode", {
              node: profileMemberName(replacedNode.remarks, replacedNode.id),
            }),
            severity: "info",
            title: t("policyGroups.switchedTitle"),
          });
        }
      },
      { inline },
    );
  }

  /**
   * The phone's tap on a group, matching its tap on a node: a running core
   * switches to the group, a stopped one only remembers it.
   */
  function selectPolicyGroup(id: string) {
    return chooseSelection(
      `${GROUP_SWITCH_PREFIX}${id}`,
      () => activatePolicyGroup(id),
      async () => {
        await voyaCommands().setActivePolicyGroup(id);
        // Groups and nodes both carry the active marker; both sit under this root.
        await refreshQueries(queryClient, queryKeys.profiles);
      },
    );
  }

  async function choosePolicyGroupMember(groupId: string, profileId: string) {
    return memberSwitch(groupId, profileId, () =>
      operation.runOperation(() => voyaCommands().selectPolicyGroupMember(groupId, profileId)),
    );
  }

  async function removePolicyGroup() {
    const entry = deletingPolicyGroup;
    if (!entry) return;
    // `runOperation` reports a failure instead of throwing; a refused second
    // press of a double click resolves to `undefined` and changes nothing.
    const removed = await runDelete(() =>
      operation.runOperation(() => voyaCommands().deletePolicyGroups([entry.group.id])),
    );
    if (removed) setDeletingPolicyGroup(null);
  }

  return {
    activatePolicyGroup,
    choosePolicyGroupMember,
    coreConnected,
    deletingPolicyGroup,
    deletingPolicyGroupPending,
    editingPolicyGroup,
    openGroupEditor,
    policyGroupEditorOpen,
    policyGroupEntries,
    policyGroupsError: policyGroupsQuery.error,
    retryPolicyGroups: () => void policyGroupsQuery.refetch(),
    policyGroupRuntimeState,
    policyGroupSubscriptions: subscriptionsQuery.data ?? [],
    removePolicyGroup,
    selectPolicyGroup,
    setDeletingPolicyGroup,
    setPolicyGroupEditorOpen,
    switchingPolicyGroupId: switchingId?.startsWith(GROUP_SWITCH_PREFIX)
      ? switchingId.slice(GROUP_SWITCH_PREFIX.length)
      : null,
    testRunningPolicyGroup: delayTest.test,
    testingPolicyGroup: delayTest.testing,
  };
}
