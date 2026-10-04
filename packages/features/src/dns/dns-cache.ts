import type { QueryClient } from "@tanstack/react-query";
import { queryKeys } from "@voya/client/query-keys";
import type { AppSettings, DnsSettings } from "@voya/contracts";

/**
 * Puts a saved DNS configuration into both caches that carry it: its own
 * query, and the copy inside the app-settings bundle.
 *
 * Reads still in flight are cancelled first. One that started before the save
 * would land after it and put the old values back on screen.
 */
export async function cacheSavedDns(client: QueryClient, saved: DnsSettings) {
  await Promise.all([
    client.cancelQueries({ queryKey: queryKeys.dns }),
    client.cancelQueries({ exact: true, queryKey: queryKeys.appSettings }),
  ]);
  client.setQueryData(queryKeys.dns, saved);
  client.setQueryData<AppSettings>(queryKeys.appSettings, (current) =>
    current ? { ...current, dns: saved } : current,
  );
}
