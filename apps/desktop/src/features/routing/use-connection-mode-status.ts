import { useQuery } from "@tanstack/react-query";

import { connectionModeStatus } from "@/ipc/commands";
import { queryKeys } from "@voya/client/query-keys";

/** The connection mode the running core reports, for the per-app proxy card and dialog. */
export function useConnectionModeStatus(enabled = true) {
  return useQuery({
    enabled,
    queryFn: connectionModeStatus,
    queryKey: queryKeys.connectionMode,
  });
}
