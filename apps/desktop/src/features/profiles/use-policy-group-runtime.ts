import { useCallback, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import type { PolicyGroupListing, PolicyGroupRuntime } from "@/ipc/bindings";
import { policyGroupRuntime, testPolicyGroupDelay } from "@/ipc/commands";
import { queryKeys } from "@voya/client/query-keys";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

/** How often a running group's live member and delays are read again; the same everywhere a group shows. */
const RUNTIME_REFRESH_MS = 3_000;

/**
 * The running policy group as the core sees it: the member traffic goes
 * through right now and each member's last measured delay. Polled only while
 * a group is active and the core is connected; a read for a group that is no
 * longer the active one never shows.
 *
 * Shared by the nodes page, the proxy groups tab and the home screen — all
 * three ride the same query key, so one fetch serves them all.
 */
export function usePolicyGroupRuntime(activeGroupId: string | null) {
  const connected = useRuntimeEventStore((state) => state.coreState?.state === "connected");
  const runtimeQuery = useQuery({
    enabled: connected && activeGroupId !== null,
    queryFn: policyGroupRuntime,
    queryKey: queryKeys.policyGroupRuntime,
    refetchInterval: RUNTIME_REFRESH_MS,
  });

  const runtime = runtimeQuery.data ?? null;
  return connected &&
    activeGroupId !== null &&
    runtime?.groupId === activeGroupId
    ? runtime
    : null;
}

/**
 * Optimistically switches the running group's selected member: the runtime and
 * listing caches move at once, and a failed commit puts the previous selection
 * back. `commit` performs the switch (usually through a `runOperation` wrapper)
 * and reports whether it saved.
 */
export function usePolicyGroupMemberSwitch() {
  const queryClient = useQueryClient();

  return useCallback(
    async (groupId: string, profileId: string, commit: () => Promise<boolean>) => {
      const previousRuntime = queryClient.getQueryData<PolicyGroupRuntime | null>(
        queryKeys.policyGroupRuntime,
      );
      const previousGroups = queryClient.getQueryData<PolicyGroupListing>(queryKeys.policyGroups);
      // Kept as the cache stored them (structural sharing may store a copy),
      // so a rollback can tell whether anything replaced them since.
      const optimisticRuntime =
        previousRuntime?.groupId === groupId
          ? queryClient.setQueryData(queryKeys.policyGroupRuntime, {
              ...previousRuntime,
              nowProfileId: profileId,
            })
          : undefined;
      const optimisticGroups = previousGroups
        ? queryClient.setQueryData<PolicyGroupListing>(queryKeys.policyGroups, {
            ...previousGroups,
            entries: previousGroups.entries.map((entry) =>
              entry.group.id === groupId
                ? { ...entry, group: { ...entry.group, selectedProfileId: profileId } }
                : entry,
            ),
          })
        : undefined;
      const saved = await commit();
      if (!saved) {
        // A runtime poll or an invalidation refetch can land during the
        // commit, and it is newer than the snapshot taken above: only a cache
        // still holding the optimistic value goes back.
        if (
          optimisticRuntime &&
          queryClient.getQueryData(queryKeys.policyGroupRuntime) === optimisticRuntime
        ) {
          queryClient.setQueryData(queryKeys.policyGroupRuntime, previousRuntime);
        }
        if (
          optimisticGroups &&
          queryClient.getQueryData(queryKeys.policyGroups) === optimisticGroups
        ) {
          queryClient.setQueryData(queryKeys.policyGroups, previousGroups);
        }
      }
      return saved;
    },
    [queryClient],
  );
}

/**
 * Re-measures every member of the running group and refreshes the runtime
 * cache with the outcome. `run` wraps the measurement the same way the caller
 * wraps its other operations, so its error reporting stays consistent.
 */
export function useGroupDelayTest(run: (operation: () => Promise<void>) => Promise<boolean>) {
  const queryClient = useQueryClient();
  const [testing, setTesting] = useState(false);

  async function test() {
    if (testing) return;
    setTesting(true);
    try {
      await run(async () => {
        const runtime = await testPolicyGroupDelay();
        if (runtime) queryClient.setQueryData(queryKeys.policyGroupRuntime, runtime);
      });
    } finally {
      setTesting(false);
    }
  }

  return { test, testing };
}
