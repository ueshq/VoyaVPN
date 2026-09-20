import { act, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale, i18next } from "@voya/i18n";
import type {
  PolicyGroupEntry,
  PolicyGroupListing,
  PolicyGroupRuntime,
  ProfileSummaryListing,
  RuntimeStatusResponse,
} from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useRuntimeActionStore } from "@/stores/runtime-action-store";
import { useToastStore } from "@/stores/toast-store";
import { makeProfileFixture } from "@/test/profile-fixture";
import { renderHookWithQuery } from "@/test/render";

import { usePolicyGroups } from "./use-policy-groups";

const ipc = vi.hoisted(() => ({
  deletePolicyGroups: vi.fn(),
  listPolicyGroups: vi.fn(),
  listSubscriptions: vi.fn(),
  policyGroupRuntime: vi.fn(),
  selectPolicyGroupMember: vi.fn(),
  setActivePolicyGroup: vi.fn(),
  testPolicyGroupDelay: vi.fn(),
}));
vi.mock("@/ipc/commands", () => ipc);

// The runtime guard and the connect/restart it drives have their own suite;
// here a switch runs the selection step and reports a connection.
const runtimeAction = vi.hoisted(() => ({ activateSelection: vi.fn() }));
vi.mock("@/stores/runtime-action", () => runtimeAction);

const connectedCore: RuntimeStatusResponse = {
  activeProfileId: null,
  activeTunBackend: null,
  connectedDurationMs: 0,
  mainPid: null,
  prePid: null,
  state: "connected",
};

function entry(id: string, isActive: boolean, selectedProfileId: string | null = "a"): PolicyGroupEntry {
  return {
    group: {
      autoCreated: false,
      id,
      intervalSeconds: null,
      memberIds: ["a", "b"],
      name: `Group ${id}`,
      selectedProfileId,
      sourceSubscriptionId: null,
      strategy: "selector",
      testUrl: null,
      toleranceMs: null,
    },
    isActive,
    members: [
      { profileId: "a", remarks: "Tokyo" },
      { profileId: "b", remarks: "Osaka" },
    ],
  };
}

function runtime(groupId: string, nowProfileId = "a"): PolicyGroupRuntime {
  return { groupId, members: [{ delayMs: 30, profileId: "a", remarks: "Tokyo" }], nowProfileId };
}

function mount(entries: PolicyGroupEntry[], runOperation = vi.fn(async (operation: () => Promise<unknown>) => {
  await operation();
  return true;
})) {
  ipc.listPolicyGroups.mockResolvedValue({ entries } satisfies PolicyGroupListing);
  const hook = renderHookWithQuery(() =>
    usePolicyGroups({ runOperation }, i18next.t.bind(i18next)),
  );
  return { ...hook, runOperation };
}

describe("usePolicyGroups", () => {
  beforeEach(async () => {
    await changeLocale("en", { persist: false });
    Object.values(ipc).forEach((mock) => mock.mockReset());
    ipc.listSubscriptions.mockResolvedValue([]);
    ipc.policyGroupRuntime.mockResolvedValue(null);
    ipc.setActivePolicyGroup.mockResolvedValue(undefined);
    ipc.selectPolicyGroupMember.mockResolvedValue(undefined);
    ipc.deletePolicyGroups.mockResolvedValue(undefined);
    runtimeAction.activateSelection
      .mockReset()
      .mockImplementation(async (_id: string, _t: unknown, select: () => Promise<void>) => {
        await select();
        return true;
      });
    useRuntimeEventStore.setState({ coreState: null });
    useRuntimeActionStore.setState({ switchingId: null });
    useToastStore.setState({ toasts: [] });
  });

  it("switches to a group and names the node it takes over from", async () => {
    const { queryClient, result } = mount([entry("g1", false)]);
    queryClient.setQueryData<ProfileSummaryListing>(queryKeys.profileList, {
      entries: [makeProfileFixture(0, { id: "tokyo", remarks: "Tokyo" }, true)],
      undecodableProfiles: 0,
    });
    await waitFor(() => expect(result.current.policyGroupEntries).toHaveLength(1));

    await act(() => result.current.activatePolicyGroup("g1"));

    expect(runtimeAction.activateSelection).toHaveBeenCalledWith("group:g1", expect.any(Function), expect.any(Function));
    expect(ipc.setActivePolicyGroup).toHaveBeenCalledWith("g1");
    expect(useToastStore.getState().toasts).toEqual([
      expect.objectContaining({
        description: "The node “Tokyo” is no longer used on its own; connections go through this group instead.",
        severity: "info",
        title: "Switched",
      }),
    ]);
  });

  it("says nothing when one group replaces another", async () => {
    const { queryClient, result } = mount([entry("g1", false), entry("g2", true)]);
    queryClient.setQueryData<ProfileSummaryListing>(queryKeys.profileList, {
      entries: [makeProfileFixture(0, { remarks: "Tokyo" }, true)],
      undecodableProfiles: 0,
    });
    await waitFor(() => expect(result.current.policyGroupEntries).toHaveLength(2));

    await act(() => result.current.activatePolicyGroup("g1"));

    expect(ipc.setActivePolicyGroup).toHaveBeenCalledWith("g1");
    expect(useToastStore.getState().toasts).toEqual([]);
  });

  it("chooses a member through the caller's operation and shows it at once", async () => {
    const { queryClient, result, runOperation } = mount([entry("g1", true)]);
    await waitFor(() => expect(result.current.policyGroupEntries).toHaveLength(1));

    let chosen!: Promise<boolean>;
    act(() => {
      chosen = result.current.choosePolicyGroupMember("g1", "b");
    });
    expect(
      queryClient.getQueryData<PolicyGroupListing>(queryKeys.policyGroups)?.entries[0]?.group.selectedProfileId,
    ).toBe("b");

    await act(async () => {
      await expect(chosen).resolves.toBe(true);
    });
    expect(runOperation).toHaveBeenCalledTimes(1);
    expect(ipc.selectPolicyGroupMember).toHaveBeenCalledWith("g1", "b");
  });

  it("follows the running group only while the core is connected", async () => {
    ipc.policyGroupRuntime.mockResolvedValue(runtime("g1", "b"));
    const { result } = mount([entry("g1", true)]);
    await waitFor(() => expect(result.current.policyGroupEntries).toHaveLength(1));
    expect(result.current.coreConnected).toBe(false);
    expect(result.current.policyGroupRuntimeState).toBeNull();
    expect(ipc.policyGroupRuntime).not.toHaveBeenCalled();

    act(() => useRuntimeEventStore.setState({ coreState: connectedCore }));

    await waitFor(() => expect(result.current.policyGroupRuntimeState).toEqual(runtime("g1", "b")));
    expect(result.current.coreConnected).toBe(true);
  });

  it("re-measures the running group and keeps what the core reports", async () => {
    ipc.testPolicyGroupDelay.mockResolvedValue(runtime("g1", "b"));
    const { queryClient, result, runOperation } = mount([entry("g1", true)]);

    await act(() => result.current.testRunningPolicyGroup());

    expect(runOperation).toHaveBeenCalledTimes(1);
    expect(queryClient.getQueryData(queryKeys.policyGroupRuntime)).toEqual(runtime("g1", "b"));
    expect(result.current.testingPolicyGroup).toBe(false);
  });

  it("deletes the group awaiting confirmation and keeps the prompt when that fails", async () => {
    let succeed = false;
    const runOperation = vi.fn(async (operation: () => Promise<unknown>) => {
      await operation();
      return succeed;
    });
    const { result } = mount([entry("g1", false)], runOperation);

    await act(() => result.current.removePolicyGroup());
    expect(ipc.deletePolicyGroups).not.toHaveBeenCalled();

    act(() => result.current.setDeletingPolicyGroup(entry("g1", false)));
    await act(() => result.current.removePolicyGroup());
    expect(ipc.deletePolicyGroups).toHaveBeenLastCalledWith(["g1"]);
    expect(result.current.deletingPolicyGroup?.group.id).toBe("g1");

    succeed = true;
    await act(() => result.current.removePolicyGroup());
    expect(result.current.deletingPolicyGroup).toBeNull();
  });

  it("opens the editor for a new group or an existing one", () => {
    const { result } = mount([]);
    expect(result.current.policyGroupEditorOpen).toBe(false);

    act(() => result.current.openGroupEditor(null));
    expect(result.current).toMatchObject({ editingPolicyGroup: null, policyGroupEditorOpen: true });

    const existing = entry("g1", false).group;
    act(() => {
      result.current.setPolicyGroupEditorOpen(false);
      result.current.openGroupEditor(existing);
    });
    expect(result.current).toMatchObject({ editingPolicyGroup: existing, policyGroupEditorOpen: true });
  });

  it("reports only a group, never a node, as the one being switched to", () => {
    const { result } = mount([]);

    act(() => useRuntimeActionStore.setState({ switchingId: "group:g1" }));
    expect(result.current.switchingPolicyGroupId).toBe("g1");

    act(() => useRuntimeActionStore.setState({ switchingId: "profile-1" }));
    expect(result.current.switchingPolicyGroupId).toBeNull();
  });
});
