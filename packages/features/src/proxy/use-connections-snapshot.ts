import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { queryKeys } from "@voya/client/query-keys";
import { proxyConnectionsPushCount, useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { voyaCommands } from "@voya/client/transport";

/**
 * The live connection table, and the read that seeds it.
 *
 * The snapshot has one source, the runtime store. The stream writes to it, and
 * so does the read here; nothing is shown from the query cache, which only
 * says whether a read is running or failed. `enabled` is whether the table is
 * wanted at all — connected, and on a phone also on screen.
 */
export function useConnectionsSnapshot(enabled: boolean) {
  const queryClient = useQueryClient();
  const snapshot = useRuntimeEventStore((state) => state.proxyConnections);
  const setProxyConnections = useRuntimeEventStore((state) => state.setProxyConnections);
  const query = useQuery({
    enabled,
    gcTime: 0,
    queryFn: async ({ signal }) => {
      const before = useRuntimeEventStore.getState().proxyConnections;
      const pushesBefore = proxyConnectionsPushCount();
      const next = await voyaCommands().proxyListConnections();
      // Cancelled while in flight: the table stopped being wanted, and a
      // screen that cleared it on the way out must not get this one back.
      if (signal.aborted) return next;
      // Seed the initial query as well as manual refreshes, but never replace a
      // newer stream event with a request that was already in flight. A pushed
      // table still waiting for its frame is such an event too, and writing
      // this answer would drop it, which is why the pushes are counted and the
      // store is not the only thing compared. The store itself refuses a table
      // whose core has gone.
      const pushed = proxyConnectionsPushCount() !== pushesBefore;
      if (useRuntimeEventStore.getState().proxyConnections === before && !pushed) {
        setProxyConnections(next);
      }
      return next;
    },
    queryKey: queryKeys.proxyConnections,
    staleTime: 3_000,
  });
  // A read stays fresh for a few seconds, so one made just before the table
  // stopped being wanted would stand in for the next session's first read.
  // Reset with it, coming back always reads again.
  useEffect(() => {
    if (!enabled) {
      void queryClient.resetQueries({ exact: true, queryKey: queryKeys.proxyConnections });
    }
  }, [enabled, queryClient]);

  return { query, setProxyConnections, snapshot };
}
