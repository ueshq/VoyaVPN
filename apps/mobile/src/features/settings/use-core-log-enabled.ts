import { useQuery } from "@tanstack/react-query";
import { queries } from "@voya/client/queries";
import type { AppSettings } from "@voya/contracts";

const coreLogEnabled = (settings: AppSettings) => settings.core.logEnabled;

/**
 * Whether the detailed connection log is switched on.
 *
 * A read of the settings every screen already shares, narrowed to the one
 * field: the screens that only say whether the log is on have no use for the
 * editing controller `useAppSettings` builds around it.
 */
export function useCoreLogEnabled() {
  return useQuery({ ...queries.appSettings, select: coreLogEnabled });
}
