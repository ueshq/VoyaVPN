import { useCallback } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import type { PolicyGroupListing, PolicyGroupRuntime } from "@voya/contracts";
import { queries } from "@voya/client/queries";
import { voyaCommands } from "@voya/client/transport";
import { queryKeys } from "@voya/client/query-keys";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

import { useBusyAction } from "../forms/use-busy-action";
import { useScreenActive } from "../shell/screen-active";

/** How often a running group's live member and delays are read again; the same everywhere a group shows. */
const RUNTIME_REFRESH_MS = 3_000;

/**
 * The running policy group as the core sees it: the member traffic goes
 * through right now and each member's last measured delay. Polled only while
 * a group is active and the core is connected; a read for a group that is no
 * longer the active one never shows.
 */
function usePolicyGroupRuntime(activeGroupId: string | null) {
  const connected = useRuntimeEventStore((state) => state.coreState?.state === "connected");
  // Read again only for a screen that is showing it; the last answer stays.
  const screenActive = useScreenActive();
  const runtimeQuery = useQuery({
    enabled: connected && activeGroupId !== null,
    queryFn: () => voyaCommands().policyGroupRuntime(),
    queryKey: queryKeys.policyGroupRuntime,
    refetchInterval: screenActive ? RUNTIME_REFRESH_MS : false,
  });

  const runtime = runtimeQuery.data ?? null;
  return connected &&
    activeGroupId !== null &&
    runtime?.groupId === activeGroupId
    ? runtime
    : null;
}

/**
 * The policy group connecting uses, if one is active, with its live state.
 *
 * Every screen that shows the active group — the nodes page, the proxy groups
 * tab, Home — asks the same two questions in the same order, so they are
 * asked here once.
 *
 * `live: false` is for a screen that lists the groups without showing the
 * running member or its delays: it skips the poll, and `runtime` stays `null`.
 */
export function useActivePolicyGroup({ live = true }: { live?: boolean } = {}) {
  const policyGroupsQuery = useQuery(queries.policyGroups);
  const activeGroup = policyGroupsQuery.data?.entries.find((entry) => entry.isActive) ?? null;
  const runtime = usePolicyGroupRuntime(live ? (activeGroup?.group.id ?? null) : null);

  return { activeGroup, policyGroupsQuery, runtime };
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
  const { busy: testing, run: once } = useBusyAction();

  async function test() {
    await once(() =>
      run(async () => {
        const runtime = await voyaCommands().testPolicyGroupDelay();
        if (runtime) queryClient.setQueryData(queryKeys.policyGroupRuntime, runtime);
      }),
    );
  }

  return { test, testing };
}
