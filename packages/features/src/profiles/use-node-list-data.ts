import { queries } from "@voya/client/queries";
import { useDeferredValue, useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import type { ProfileSummaryEntry, SpeedtestResult } from "@voya/contracts";
import type { TranslationFunction } from "@voya/i18n/core";
import { metadataBySubscriptionId } from "../subscriptions/subscription-usage";
import { nodeListRows, nodeSearchText } from "./node-list-rows";

/**
 * What the list view is showing, as the persisted node-list store holds it.
 * Structural rather than the desktop hook's return type: the same shaping
 * feeds a React Native `SectionList`.
 */
export type NodeListSelection = {
  collapsed: ReadonlySet<string>;
  hideUnreachable: boolean;
  search: string;
  sortByLatency: boolean;
};

/**
 * The node list's data: the queries behind it, the live speedtest overlay and
 * the rows they add up to. Laying the rows out on a screen — virtualized on
 * the desktop, sectioned on mobile — is the view's business.
 */
export function useNodeListData(
  nodeGroups: NodeListSelection,
  t: TranslationFunction,
) {
  const metadataQuery = useQuery(queries.subscriptionMetadata);
  const subscriptionMetadata = useMemo(
    () => metadataBySubscriptionId(metadataQuery.data ?? []),
    [metadataQuery.data],
  );
  const subscriptionsQuery = useQuery(queries.subscriptions);
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
  const profilesQuery = useQuery(queries.profileList);
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
    // The deferred search the rows were built from; a view that scrolls resets
    // its position on it rather than on every keystroke.
    search,
    speedtestResultsByProfileId,
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
