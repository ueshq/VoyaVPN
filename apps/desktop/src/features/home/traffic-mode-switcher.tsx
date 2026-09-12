import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";
import { cn } from "@voya/ui/lib/utils";
import { getErrorMessage } from "@voya/utils/error";
import { loadAppSettings, proxySetTrafficMode } from "@/ipc/commands";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { AppSettingsV1, TrafficMode } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import {
  runtimeActionPending,
  useRuntimeActionStore,
} from "@/stores/runtime-action-store";
import { ModeInfo } from "./mode-info";

const modes = [
  { value: "rule", labelKey: "home.trafficModeSmart" },
  { value: "global", labelKey: "proxy.trafficModeGlobal" },
] as const;

export function TrafficModeSwitcher() {
  const { t } = useI18n();
  const client = useQueryClient();
  const state = useRuntimeEventStore((store) => store.coreState?.state);
  const pending = useRuntimeActionStore(runtimeActionPending);
  const query = useQuery({
    queryKey: queryKeys.appSettings,
    queryFn: loadAppSettings,
  });
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
  const disabled = !ready || pending || !query.data || query.isError;

  function selectMode(mode: TrafficMode) {
    if (disabled || runtimeActionPending()) return;
    useRuntimeActionStore.setState({ modePending: true });
    mutation.mutate(mode);
  }

  return (
    <div className="home-traffic-mode">
      <div className="home-mode-row">
        <div className="home-mode-label">
          <span id="home-traffic-mode-label">{t("home.trafficMode")}</span>
          <ModeInfo
            label={t("home.trafficModeInfo")}
            hint={t("home.trafficModeHint")}
          />
        </div>
        <div
          aria-labelledby="home-traffic-mode-label"
          className="home-traffic-mode-options"
          role="group"
        >
          {modes.map(({ value, labelKey }) => (
            <Button
              aria-pressed={query.data?.proxy.trafficMode === value}
              className={cn(
                "h-8 px-4 text-xs text-foreground",
                query.data?.proxy.trafficMode === value &&
                  "bg-background text-foreground shadow-sm",
              )}
              disabled={disabled}
              key={value}
              onClick={() => selectMode(value)}
              type="button"
              variant="ghost"
            >
              {t(labelKey)}
            </Button>
          ))}
        </div>
      </div>
      {query.error ? (
        <div className="home-mode-hint" role="alert">
          {getErrorMessage(query.error)}
          <Button
            onClick={() => void query.refetch()}
            size="sm"
            type="button"
            variant="ghost"
          >
            {t("actions.retry")}
          </Button>
        </div>
      ) : null}
    </div>
  );
}
