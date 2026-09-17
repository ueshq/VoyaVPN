import { useEffect, useMemo, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { listProfiles, listSubscriptions, listSubscriptionMetadata } from "@/ipc/commands";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { firstPaintVirtualItems } from "@/lib/virtual-list";
import { queryKeys } from "@/ipc/query-keys";
import type { ProfileListEntry, SpeedtestResult } from "@/ipc/bindings";
import type { TranslationFunction } from "@voya/i18n";
import { metadataBySubscriptionId } from "@/features/subscriptions/subscription-usage";
import { nodeListRows } from "./node-list-rows";
import type { useNodeGroups } from "./use-node-groups";

export function useNodeListData(
  nodeGroups: ReturnType<typeof useNodeGroups>,
  t: TranslationFunction,
) {
  const metadataQuery = useQuery({
    queryFn: listSubscriptionMetadata,
    queryKey: queryKeys.subscriptionMetadata,
  });
  const subscriptionMetadata = useMemo(
    () => metadataBySubscriptionId(metadataQuery.data ?? []),
    [metadataQuery.data],
  );
  const subscriptionsQuery = useQuery({
    queryFn: listSubscriptions,
    queryKey: queryKeys.subscriptions,
  });
  const subscriptionNames = useMemo(
    () =>
      new Map(
        (subscriptionsQuery.data ?? []).map((item) => [
          item.id,
          item.remarks || t("panes.subscriptions.untitled"),
        ]),
      ),
    [subscriptionsQuery.data, t],
  );
  const speedtestResultsByProfileId = useRuntimeEventStore(
    (state) => state.speedtestResultsByProfileId,
  );
  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, null),
    queryKey: queryKeys.profileList,
  });
  const profiles = useMemo(
    () =>
      applySpeedtestResults(
        profilesQuery.data?.entries ?? [],
        speedtestResultsByProfileId,
      ),
    [profilesQuery.data, speedtestResultsByProfileId],
  );
  // Stored servers this build could not read. The backend skips those rows so
  // one of them cannot hide every other server, and reports how many it
  // skipped; the toolbar states it, because a list that is quietly short is
  // indistinguishable from data loss.
  const undecodableProfiles = profilesQuery.data?.undecodableProfiles ?? 0;

  const viewportRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (viewportRef.current) viewportRef.current.scrollTop = 0;
  }, [nodeGroups.search]);
  const rows = useMemo(
    () =>
      nodeListRows(
        profiles,
        nodeGroups.collapsed,
        t("nodeGroups.local"),
        subscriptionsQuery.data,
        t("panes.subscriptions.untitled"),
        {
          search: nodeGroups.search,
          hideUnreachable: nodeGroups.hideUnreachable,
          sortByLatency: nodeGroups.sortByLatency,
        },
      ),
    [
      profiles,
      nodeGroups.collapsed,
      nodeGroups.search,
      nodeGroups.hideUnreachable,
      nodeGroups.sortByLatency,
      subscriptionsQuery.data,
      t,
    ],
  );
  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    // Compact rows are about 56 px and group headers a little taller.
    estimateSize: () => 64,
    getItemKey: (index) => rows[index]!.key,
    getScrollElement: () => viewportRef.current,
    initialRect: { height: 520, width: 1200 },
    overscan: 5,
  });
  const renderedRows = firstPaintVirtualItems(
    rowVirtualizer.getVirtualItems(),
    rows.length,
    64,
    15,
    (index) => rows[index]!.key,
  );
  function subscriptionName(item: ProfileListEntry) {
    return item.profile.subscriptionId
      ? (subscriptionNames.get(item.profile.subscriptionId) ??
          t("panes.subscriptions.untitled"))
      : t("panes.profiles.card.local");
  }

  return {
    visibleProfileCount: rows.reduce((count, row) => count + (row.kind === "group" ? row.members.length : 0), 0),
    profiles,
    profilesQuery,
    rows,
    renderedRows,
    rowVirtualizer,
    viewportRef,
    subscriptionsQuery,
    subscriptionMetadata,
    subscriptionName,
    undecodableProfiles,
  };
}

export function applySpeedtestResults(
  profiles: ProfileListEntry[],
  speedtestResults: Record<string, SpeedtestResult>,
) {
  if (Object.keys(speedtestResults).length === 0) return profiles;

  let changed = false;
  const nextProfiles = profiles.map((item) => {
    const result = speedtestResults[item.profile.id];
    if (!result) return item;

    changed = true;
    return {
      ...item,
      metrics: {
        ...item.metrics,
        // Country stays on the query snapshot; cached events may outlive edits.
        delayMs: result.delay ?? item.metrics.delayMs,
        ipInfo: result.ipInfo ?? item.metrics.ipInfo,
        outcome: result.outcome,
      },
    };
  });

  return changed ? nextProfiles : profiles;
}
