import { useQuery } from "@tanstack/react-query";

import { voyaCommands } from "@voya/client/transport";
import { queryKeys } from "@voya/client/query-keys";

/** The connection mode the running core reports, for the per-app proxy card and dialog. */
export function useConnectionModeStatus(enabled = true) {
  return useQuery({
    enabled,
    queryFn: () => voyaCommands().connectionModeStatus(),
    queryKey: queryKeys.connectionMode,
  });
}
