import { queries } from "@voya/client/queries";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";

/**
 * What the running connection still owes the saved settings.
 *
 * Read once per mount, and again whenever `refreshKey` changes to a non-null
 * value: the answer goes stale when a save lands or the core changes, not on a
 * timer. Mounting is the query's own fetch, so the effect skips its first run —
 * asking twice on mount sent two commands for one answer.
 */
export function useSettingsApplyStatus({
  enabled = true,
  refreshKey,
}: {
  enabled?: boolean;
  /** Whatever the status depends on; `null` while a refresh would be premature. */
  refreshKey: string | null;
}) {
  const queryClient = useQueryClient();
  const query = useQuery({
    ...queries.settingsApply,
    enabled,
    refetchOnMount: "always",
  });
  const { refetch } = query;
  const seenKey = useRef(refreshKey);
  const wasEnabled = useRef(enabled);

  useEffect(() => {
    const justEnabled = enabled && !wasEnabled.current;
    wasEnabled.current = enabled;
    if (seenKey.current === refreshKey) return;
    seenKey.current = refreshKey;
    if (refreshKey === null) return;
    // A caller that enables the query as the key appears — Home, on connect —
    // has had the fetch started by the enabling itself; asking again here
    // would cancel that one and send the command a second time.
    const { queryKey } = queries.settingsApply;
    if (justEnabled && queryClient.isFetching({ queryKey }) > 0) return;
    void refetch();
  }, [enabled, queryClient, refreshKey, refetch]);

  return query;
}
