import { queries } from "@voya/client/queries";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type { TranslationKey } from "@voya/i18n/core";
import { useI18n } from "@voya/i18n/use-i18n";
import { voyaCommands } from "@voya/client/transport";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import type { AppSettings, TrafficMode } from "@voya/contracts";
import { queryKeys } from "@voya/client/query-keys";
import { runtimeActionPending, useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useRuntimeBusy } from "@voya/client/runtime-action";

/**
 * The saved traffic mode. The Rules page locks its rules while it is global,
 * which routes all captured traffic ahead of every rule.
 */
export function useSavedTrafficMode() {
  const query = useQuery(queries.appSettings);

  return {
    error: query.error,
    mode: query.data?.proxy.trafficMode,
    retry: () => void query.refetch(),
  };
}

/** The saved traffic mode and the one way to change it. */
export function useTrafficMode() {
  const { t } = useI18n();
  const client = useQueryClient();
  const state = useRuntimeEventStore((store) => store.coreState?.state);
  const busy = useRuntimeBusy();
  const { error, mode } = useSavedTrafficMode();
  const mutation = useMutation({
    mutationFn: (mode: TrafficMode) => voyaCommands().proxySetTrafficMode(mode),
    meta: { errorTitle: t("proxy.trafficModeFailed") },
    onSuccess: async ({ mode }) => {
      // An invalidation read may still be in flight when the command returns.
      await client.cancelQueries({ queryKey: queryKeys.appSettings });
      client.setQueryData<AppSettings>(queryKeys.appSettings, (current) =>
        current
          ? { ...current, proxy: { ...current.proxy, trafficMode: mode } }
          : current,
      );
    },
    onSettled: async () => {
      // Persistence can succeed before the live update fails. Reconcile both
      // the saved choice and the connection list on either outcome.
      try {
        await Promise.all([
          client.invalidateQueries({ queryKey: queryKeys.appSettings }),
          client.invalidateQueries({ queryKey: queryKeys.proxyConnections }),
        ]);
      } finally {
        useRuntimeActionStore.getState().setModePending(false);
      }
    },
  });
  const ready = state === "connected" || state === "disconnected";
  const disabled = !ready || busy || mode === undefined || error !== null;
  // A disabled button cannot say why, so the switcher shows this on hover.
  const disabledReason: TranslationKey | null =
    error !== null
      ? "panes.routing.trafficModeUnavailable"
      : !ready || busy
        ? "common.waitForConnection"
        : null;

  function selectMode(mode: TrafficMode) {
    if (disabled || runtimeActionPending()) return;
    useRuntimeActionStore.getState().setModePending(true);
    mutation.mutate(mode);
  }

  return { disabled, disabledReason, mode, selectMode };
}
