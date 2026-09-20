import { useDeferredValue, useEffect, useMemo, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { listProfileSummaries, listSubscriptions, listSubscriptionMetadata } from "@/ipc/commands";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { firstPaintVirtualItems } from "@/lib/virtual-list";
import { queryKeys } from "@voya/client/query-keys";
import type { ProfileSummaryEntry, SpeedtestResult } from "@/ipc/bindings";
import type { TranslationFunction } from "@voya/i18n";
import { metadataBySubscriptionId } from "@/features/subscriptions/subscription-usage";
import { nodeListRows, nodeSearchText } from "./node-list-rows";
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
    queryFn: () => listProfileSummaries(),
    queryKey: queryKeys.profileList,
  });
  const base = profilesQuery.data?.entries ?? NO_PROFILES;
  const profiles = useMemo(
    () => applySpeedtestResults(base, speedtestResultsByProfileId),
    [base, speedtestResultsByProfileId],
  );
  // A speedtest delivers results many times a second. While neither the sort
  // nor the filter reads them, the rows are laid out from the listing alone,
  // so a result frame rebuilds no groups; the grid overlays the few rows it
  // renders instead.
  const metricsDriveLayout = nodeGroups.sortByLatency || nodeGroups.hideUnreachable;
  const layoutProfiles = metricsDriveLayout ? profiles : base;
  // Typing stays responsive in a large list: the rows follow a step behind.
  const search = useDeferredValue(nodeGroups.search);
  const searching = search.trim() !== "";
  // Each node's text is lowercased once per listing, not once per keystroke.
  const searchHays = useMemo(
    () => (searching ? new Map(base.map((item) => [item.profile.id, nodeSearchText(item)])) : undefined),
    [base, searching],
  );
  // Stored servers this build could not read. The backend skips those rows so
  // one of them cannot hide every other server, and reports how many it
  // skipped; the toolbar states it, because a list that is quietly short is
  // indistinguishable from data loss.
  const undecodableProfiles = profilesQuery.data?.undecodableProfiles ?? 0;

  const viewportRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (viewportRef.current) viewportRef.current.scrollTop = 0;
  }, [search]);
  const rows = useMemo(
    () =>
      nodeListRows(
        layoutProfiles,
        nodeGroups.collapsed,
        t("nodeGroups.local"),
        subscriptionsQuery.data,
        t("panes.subscriptions.untitled"),
        {
          search,
          searchHays,
          hideUnreachable: nodeGroups.hideUnreachable,
          sortByLatency: nodeGroups.sortByLatency,
        },
      ),
    [
      layoutProfiles,
      nodeGroups.collapsed,
      search,
      searchHays,
      nodeGroups.hideUnreachable,
      nodeGroups.sortByLatency,
      subscriptionsQuery.data,
      t,
    ],
  );
  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Virtual exposes scroll helpers that React Compiler cannot memoize safely.
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
  function subscriptionName(item: ProfileSummaryEntry) {
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
    speedtestResultsByProfileId,
    renderedRows,
    rowVirtualizer,
    viewportRef,
    subscriptionsQuery,
    subscriptionMetadata,
    subscriptionName,
    undecodableProfiles,
  };
}

const NO_PROFILES: ProfileSummaryEntry[] = [];

export function applySpeedtestResults(
  profiles: ProfileSummaryEntry[],
  speedtestResults: Record<string, SpeedtestResult>,
) {
  if (Object.keys(speedtestResults).length === 0) return profiles;

  let changed = false;
  const nextProfiles = profiles.map((item) => {
    const next = overlaySpeedtestResult(item, speedtestResults[item.profile.id]);
    changed ||= next !== item;
    return next;
  });

  return changed ? nextProfiles : profiles;
}

/**
 * Overlays built so far, by result. The store keeps a result's object until a
 * newer one replaces it, so each result is overlaid once however many frames
 * follow, and the row keeps its identity between them.
 */
const overlays = new WeakMap<
  SpeedtestResult,
  { entry: ProfileSummaryEntry; item: ProfileSummaryEntry }
>();

/**
 * `item` with a live speedtest result on top. Idempotent: `item` itself when
 * there is no result or it already reads as one, and the same object for the
 * same pair.
 */
export function overlaySpeedtestResult(
  item: ProfileSummaryEntry,
  result: SpeedtestResult | undefined,
): ProfileSummaryEntry {
  if (!result) return item;
  const cached = overlays.get(result);
  if (cached?.item === item) return cached.entry;
  // Country stays on the query snapshot; cached events may outlive edits.
  const delayMs = result.delay ?? item.metrics.delayMs;
  const ipInfo = result.ipInfo ?? item.metrics.ipInfo;
  const { outcome } = result;
  if (
    delayMs === item.metrics.delayMs &&
    ipInfo === item.metrics.ipInfo &&
    outcome === item.metrics.outcome
  ) {
    return item;
  }
  const entry = { ...item, metrics: { ...item.metrics, delayMs, ipInfo, outcome } };
  overlays.set(result, { entry, item });
  return entry;
}
