import { queryOptions, type QueryClient } from "@tanstack/react-query";

import { queryKeys, type QueryKeyRoot } from "./query-keys";
import { voyaCommands } from "./transport";

/**
 * Each shared cache's key bound to the command that fills it.
 *
 * A key written beside a hand-picked fetcher in every `useQuery` is how two
 * screens end up caching different data under one key; spreading one of these
 * (`useQuery({ ...queries.routings, select })`) makes that impossible.
 */
export const queries = {
  appSettings: queryOptions({
    queryKey: queryKeys.appSettings,
    queryFn: () => voyaCommands().loadAppSettings(),
  }),
  connectionMode: queryOptions({
    queryKey: queryKeys.connectionMode,
    queryFn: () => voyaCommands().connectionModeStatus(),
  }),
  dns: queryOptions({
    queryKey: queryKeys.dns,
    queryFn: () => voyaCommands().loadDnsSettings(),
  }),
  policyGroups: queryOptions({
    queryKey: queryKeys.policyGroups,
    queryFn: () => voyaCommands().listPolicyGroups(),
  }),
  profileList: queryOptions({
    queryKey: queryKeys.profileList,
    queryFn: () => voyaCommands().listProfileSummaries(),
  }),
  routings: queryOptions({
    queryKey: queryKeys.routings,
    queryFn: () => voyaCommands().listRoutings(),
  }),
  settingsApply: queryOptions({
    queryKey: queryKeys.settingsApply,
    queryFn: () => voyaCommands().getSettingsApplyStatus(),
  }),
  subscriptionMetadata: queryOptions({
    queryKey: queryKeys.subscriptionMetadata,
    queryFn: () => voyaCommands().listSubscriptionMetadata(),
  }),
  subscriptions: queryOptions({
    queryKey: queryKeys.subscriptions,
    queryFn: () => voyaCommands().listSubscriptions(),
  }),
  uiPreferences: queryOptions({
    queryKey: queryKeys.uiPreferences,
    queryFn: () => voyaCommands().loadUiPreferences(),
  }),
};

/**
 * Refetches what is mounted under each root and resolves once it has settled.
 *
 * The backend's invalidation event refreshes the same caches a moment later;
 * this is for the screen that must not navigate, or close, before it holds the
 * data its own write changed. Naming the roots is the point: an unscoped
 * invalidation refetches every mounted query in the app, including ones no
 * write touches — the exit-address lookup on Home, an external request.
 *
 * A fetch already in flight is cancelled and restarted, never joined: it may
 * have been answered before the write landed.
 */
export async function refreshQueries(client: QueryClient, ...roots: readonly QueryKeyRoot[]) {
  await Promise.all(roots.map((queryKey) => client.invalidateQueries({ queryKey })));
}
