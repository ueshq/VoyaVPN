import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { voyaCommands } from "@voya/client/transport";
import { connectionIpQueryKey, queryKeys } from "@voya/client/query-keys";
import { type RuntimeEventState, useRuntimeEventStore } from "@voya/client/runtime-event-store";

/**
 * Names the running connection; `null` while there is none.
 *
 * The node and the core process say which connection this is wherever the
 * core is a process. Inside the system's tunnel provider it has no process id,
 * so the session's connection count is what tells a reconnect to the same node
 * from the connection before it.
 */
export function runningConnectionKey(
  state: Pick<RuntimeEventState, "connectionEpoch" | "coreState">,
): string | null {
  const core = state.coreState;
  return core?.state === "connected"
    ? `${core.activeProfileId ?? ""}:${core.mainPid ?? ""}:${state.connectionEpoch}`
    : null;
}

/** How often a failed lookup is tried again before the failure is shown. */
const LOOKUP_RETRIES = 2;
const LOOKUP_RETRY_DELAY_MS = 3000;
/** How long a shown failure stands before the lookup starts over. */
const FAILED_LOOKUP_INTERVAL_MS = 30_000;

/**
 * The exit address of the running connection. The query key follows the
 * connection itself (`connectionKey`), so a result never outlives the
 * connection it describes. The lookup runs on every connect, and there is
 * nothing to press: a failure is retried here, because the first seconds of a
 * tunnel are when a lookup through it is most likely to fail. Home's exit IP
 * metric and map share this query.
 * The cached result survives leaving and re-entering Home (screens unmount on
 * tab switch), but reconnecting is a new connection, hence a new key, and a
 * new lookup.
 *
 * A failure that outlasts those retries is not final either. A phone keeps
 * Home mounted for the whole connection, so nothing would ever ask again: the
 * lookup starts over every half minute while it stands failed, and when the
 * app comes back to the front. An answer is never looked up twice — it is
 * never stale, and the interval only runs on a failure.
 *
 * That survival is why the result is never garbage-collected by age: with Home
 * unmounted nothing observes it, and a timer would drop the answer the user
 * comes back to. What ends a result is its connection, so every other
 * connection's entry is removed when the key changes — the cache holds one.
 */
export function useConnectionIp() {
  const queryClient = useQueryClient();
  const connectionKey = useRuntimeEventStore(runningConnectionKey);
  const ipQuery = useQuery({
    enabled: connectionKey !== null,
    gcTime: Number.POSITIVE_INFINITY,
    queryFn: () => voyaCommands().checkConnectionIp(),
    queryKey: connectionIpQueryKey(connectionKey),
    refetchInterval: (query) => (query.state.status === "error" ? FAILED_LOOKUP_INTERVAL_MS : false),
    refetchOnWindowFocus: true,
    retry: LOOKUP_RETRIES,
    retryDelay: (attempt) => LOOKUP_RETRY_DELAY_MS * (attempt + 1),
    staleTime: Number.POSITIVE_INFINITY,
  });
  useEffect(() => {
    queryClient.removeQueries({
      predicate: (query) => query.queryKey[1] !== connectionKey,
      queryKey: queryKeys.connectionIp,
    });
  }, [connectionKey, queryClient]);
  return ipQuery;
}
