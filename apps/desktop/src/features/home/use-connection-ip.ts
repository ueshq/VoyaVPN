import { useQuery } from "@tanstack/react-query";

import { checkConnectionIp, loadAppSettings } from "@/ipc/commands";
import { connectionIpQueryKey, queryKeys } from "@voya/client/query-keys";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

/**
 * The exit address of the running connection. The query key follows the
 * connection itself (node and process), so a result never outlives the
 * connection it describes; the lookup runs on connect only when the user
 * asked for it in settings. Home's exit IP metric and map share this query.
 * The cached result survives leaving and re-entering Home (screens unmount on
 * tab switch), but reconnecting spawns a new core process, hence a new key,
 * and returns the metric to "not checked".
 */
export function useConnectionIp() {
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const connectionKey =
    coreState?.state === "connected"
      ? `${coreState.activeProfileId ?? ""}:${coreState.mainPid ?? ""}`
      : null;
  const settingsQuery = useQuery({
    queryFn: () => loadAppSettings(),
    queryKey: queryKeys.appSettings,
  });
  const autoCheck = settingsQuery.data?.behavior.autoCheckIp ?? false;
  const ipQuery = useQuery({
    enabled: connectionKey !== null && autoCheck,
    gcTime: Number.POSITIVE_INFINITY,
    queryFn: () => checkConnectionIp(),
    queryKey: connectionIpQueryKey(connectionKey),
    retry: false,
    staleTime: Number.POSITIVE_INFINITY,
  });
  return { connectionKey, ipQuery };
}
