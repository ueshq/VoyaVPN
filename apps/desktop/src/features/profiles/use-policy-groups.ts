import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import type { TranslationFunction } from "@voya/i18n";
import type { PolicyGroup, PolicyGroupEntry, ProfileListing } from "@/ipc/bindings";
import {
  deletePolicyGroups,
  listPolicyGroups,
  listSubscriptions,
  selectPolicyGroupMember,
  setActivePolicyGroup,
} from "@/ipc/commands";
import { queryKeys } from "@/ipc/query-keys";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useRuntimeActionStore } from "@/stores/runtime-action-store";
import { useToastStore } from "@/stores/toast-store";
import { activateSelection } from "@/stores/runtime-action";

import { profileMemberName } from "./profile-display";
import {
  useGroupDelayTest,
  usePolicyGroupMemberSwitch,
  usePolicyGroupRuntime,
} from "./use-policy-group-runtime";
import type { NodeOperation } from "./use-node-operation";

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
  const activeGroupId =
    policyGroupEntries.find((entry) => entry.isActive)?.group.id ?? null;
  const policyGroupRuntimeState = usePolicyGroupRuntime(activeGroupId);
  const memberSwitch = usePolicyGroupMemberSwitch();
  const delayTest = useGroupDelayTest(operation.runOperation);
  const subscriptionsQuery = useQuery({
    queryFn: listSubscriptions,
    queryKey: queryKeys.subscriptions,
  });
  const [editingPolicyGroup, setEditingPolicyGroup] = useState<PolicyGroup | null>(null);
  const [policyGroupEditorOpen, setPolicyGroupEditorOpen] = useState(false);
  const [deletingPolicyGroup, setDeletingPolicyGroup] = useState<PolicyGroupEntry | null>(null);

  function openGroupEditor(group: PolicyGroup | null) {
    setEditingPolicyGroup(group);
    setPolicyGroupEditorOpen(true);
  }

  /** The same two steps as choosing a node: make it active, then connect or restart. */
  function activatePolicyGroup(id: string) {
    return activateSelection(`${GROUP_SWITCH_PREFIX}${id}`, t, async () => {
      // A group replaces a node used on its own; say which one it set aside.
      const replacedNode = policyGroupEntries.some((entry) => entry.isActive)
        ? null
        : queryClient
            .getQueryData<ProfileListing>(queryKeys.profileList)
            ?.entries.find((entry) => entry.isActive)?.profile;
      await setActivePolicyGroup(id);
      if (replacedNode) {
        useToastStore.getState().pushToast({
          description: t("policyGroups.replacedNode", {
            node: profileMemberName(replacedNode.remarks, replacedNode.id),
          }),
          severity: "info",
          title: t("policyGroups.switchedTitle"),
        });
      }
    });
  }

  async function choosePolicyGroupMember(groupId: string, profileId: string) {
    return memberSwitch(groupId, profileId, () =>
      operation.runOperation(() => selectPolicyGroupMember(groupId, profileId)),
    );
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
    policyGroupRuntimeState,
    policyGroupSubscriptions: subscriptionsQuery.data ?? [],
    removePolicyGroup,
    setDeletingPolicyGroup,
    setPolicyGroupEditorOpen,
    switchingPolicyGroupId: switchingId?.startsWith(GROUP_SWITCH_PREFIX)
      ? switchingId.slice(GROUP_SWITCH_PREFIX.length)
      : null,
    testRunningPolicyGroup: delayTest.test,
    testingPolicyGroup: delayTest.testing,
  };
}
