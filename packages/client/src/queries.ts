import { queryOptions } from "@tanstack/react-query";

import { queryKeys } from "./query-keys";
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
