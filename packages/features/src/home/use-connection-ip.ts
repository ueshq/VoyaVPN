import { queries } from "@voya/client/queries";
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

/**
 * The exit address of the running connection. The query key follows the
 * connection itself (`connectionKey`), so a result never outlives the
 * connection it describes; the lookup runs on connect only when the user
 * asked for it in settings. Home's exit IP metric and map share this query.
 * The cached result survives leaving and re-entering Home (screens unmount on
 * tab switch), but reconnecting is a new connection, hence a new key, and
 * returns the metric to "not checked".
 *
 * That survival is why the result is never garbage-collected by age: with Home
 * unmounted nothing observes it, and a timer would drop the answer the user
 * comes back to. What ends a result is its connection, so every other
 * connection's entry is removed when the key changes — the cache holds one.
 */
export function useConnectionIp() {
  const queryClient = useQueryClient();
  const connectionKey = useRuntimeEventStore(runningConnectionKey);
  const settingsQuery = useQuery(queries.appSettings);
  const autoCheck = settingsQuery.data?.behavior.autoCheckIp ?? false;
  const ipQuery = useQuery({
    enabled: connectionKey !== null && autoCheck,
    gcTime: Number.POSITIVE_INFINITY,
    queryFn: () => voyaCommands().checkConnectionIp(),
    queryKey: connectionIpQueryKey(connectionKey),
    staleTime: Number.POSITIVE_INFINITY,
  });
  useEffect(() => {
    queryClient.removeQueries({
      predicate: (query) => query.queryKey[1] !== connectionKey,
      queryKey: queryKeys.connectionIp,
    });
  }, [connectionKey, queryClient]);
  return { connectionKey, ipQuery };
}
