import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { useI18n } from "@voya/i18n/use-i18n";
import { loadAppSettings, proxySetTrafficMode } from "@/ipc/commands";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { AppSettingsV1, TrafficMode } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import {
  runtimeActionPending,
  useRuntimeActionStore,
} from "@/stores/runtime-action-store";

/**
 * The saved traffic mode. The Rules page locks its rules while it is global,
 * which routes all captured traffic ahead of every rule.
 */
export function useSavedTrafficMode() {
  const query = useQuery({
    queryKey: queryKeys.appSettings,
    queryFn: loadAppSettings,
  });

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
  const pending = useRuntimeActionStore(runtimeActionPending);
  const { error, mode } = useSavedTrafficMode();
  const mutation = useMutation({
    mutationFn: proxySetTrafficMode,
    meta: { errorTitle: t("proxy.trafficModeFailed") },
    onSuccess: async ({ mode }) => {
      // An invalidation read may still be in flight when the command returns.
      await client.cancelQueries({ queryKey: queryKeys.appSettings });
      client.setQueryData<AppSettingsV1>(queryKeys.appSettings, (current) =>
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
        useRuntimeActionStore.setState({ modePending: false });
      }
    },
  });
  const ready = state === "connected" || state === "disconnected";
  const disabled = !ready || pending || mode === undefined || error !== null;

  function selectMode(mode: TrafficMode) {
    if (disabled || runtimeActionPending()) return;
    useRuntimeActionStore.setState({ modePending: true });
    mutation.mutate(mode);
  }

  return { disabled, mode, selectMode };
}
