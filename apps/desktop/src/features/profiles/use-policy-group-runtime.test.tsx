import { act } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { PolicyGroupEntry, PolicyGroupListing, PolicyGroupRuntime } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { renderHookWithQuery } from "@/test/render";

import { usePolicyGroupMemberSwitch } from "./use-policy-group-runtime";

function runtime(nowProfileId: string, groupId = "g1"): PolicyGroupRuntime {
  return {
    groupId,
    members: [
      { delayMs: 40, profileId: "a", remarks: "Tokyo" },
      { delayMs: 90, profileId: "b", remarks: "Osaka" },
    ],
    nowProfileId,
  };
}

function listing(selectedProfileId: string): PolicyGroupListing {
  const entry: PolicyGroupEntry = {
    group: {
      autoCreated: false,
      id: "g1",
      intervalSeconds: null,
      memberIds: ["a", "b"],
      name: "Asia",
      selectedProfileId,
      sourceSubscriptionId: null,
      strategy: "selector",
      testUrl: null,
      toleranceMs: null,
    },
    isActive: true,
    members: [
      { profileId: "a", remarks: "Tokyo" },
      { profileId: "b", remarks: "Osaka" },
    ],
  };
  return { entries: [entry] };
}

function mountSwitch() {
  const hook = renderHookWithQuery(() => usePolicyGroupMemberSwitch());
  hook.queryClient.setQueryData(queryKeys.policyGroupRuntime, runtime("a"));
  hook.queryClient.setQueryData(queryKeys.policyGroups, listing("a"));
  const cached = () => ({
    groups: hook.queryClient.getQueryData<PolicyGroupListing>(queryKeys.policyGroups),
    runtime: hook.queryClient.getQueryData<PolicyGroupRuntime>(queryKeys.policyGroupRuntime),
  });
  return { ...hook, cached };
}

function pendingCommit() {
  let finish!: (saved: boolean) => void;
  const commit = () =>
    new Promise<boolean>((resolve) => {
      finish = resolve;
    });
  return { commit, finish: (saved: boolean) => finish(saved) };
}

describe("policy group member switch", () => {
  it("moves both caches at once and keeps them when the commit saves", async () => {
    const { cached, result } = mountSwitch();
    const { commit, finish } = pendingCommit();

    let switched!: Promise<boolean>;
    act(() => {
      switched = result.current("g1", "b", commit);
    });
    expect(cached().runtime?.nowProfileId).toBe("b");
    expect(cached().groups?.entries[0]?.group.selectedProfileId).toBe("b");

    await act(async () => finish(true));
    await expect(switched).resolves.toBe(true);
    expect(cached().runtime?.nowProfileId).toBe("b");
    expect(cached().groups?.entries[0]?.group.selectedProfileId).toBe("b");
  });

  it("puts the previous selection back when the commit fails and nothing newer landed", async () => {
    const { cached, result } = mountSwitch();
    const { commit, finish } = pendingCommit();

    let switched!: Promise<boolean>;
    act(() => {
      switched = result.current("g1", "b", commit);
    });
    await act(async () => finish(false));

    await expect(switched).resolves.toBe(false);
    expect(cached().runtime).toEqual(runtime("a"));
    expect(cached().groups).toEqual(listing("a"));
  });

  it("keeps a poll and a refetch that landed during a failed commit", async () => {
    const { cached, queryClient, result } = mountSwitch();
    const { commit, finish } = pendingCommit();

    let switched!: Promise<boolean>;
    act(() => {
      switched = result.current("g1", "b", commit);
    });
    // The 3 s runtime poll and a listing refetch report what the core and the
    // database hold now; rolling back would overwrite them with older data.
    const polled = { ...runtime("a"), members: [{ delayMs: 12, profileId: "a", remarks: "Tokyo" }] };
    const refetched = { entries: [{ ...listing("a").entries[0]!, isActive: false }] };
    act(() => {
      queryClient.setQueryData(queryKeys.policyGroupRuntime, polled);
      queryClient.setQueryData(queryKeys.policyGroups, refetched);
    });
    await act(async () => finish(false));

    await expect(switched).resolves.toBe(false);
    expect(cached().runtime).toEqual(polled);
    expect(cached().groups).toEqual(refetched);
  });

  it("leaves the runtime of another group alone", async () => {
    const { cached, queryClient, result } = mountSwitch();
    queryClient.setQueryData(queryKeys.policyGroupRuntime, runtime("a", "g2"));
    const { commit, finish } = pendingCommit();

    let switched!: Promise<boolean>;
    act(() => {
      switched = result.current("g1", "b", commit);
    });
    expect(cached().runtime).toEqual(runtime("a", "g2"));
    await act(async () => finish(false));

    await expect(switched).resolves.toBe(false);
    expect(cached().runtime).toEqual(runtime("a", "g2"));
    expect(cached().groups).toEqual(listing("a"));
  });
});
