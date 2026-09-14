import { useQuery } from "@tanstack/react-query";
import { LoaderCircle, RefreshCw } from "lucide-react";

import type { TranslationFunction } from "@voya/i18n";
import { Button } from "@voya/ui/components/button";

import { DisabledReason } from "@/components/disabled-reason";
import { checkConnectionIp, loadAppSettings } from "@/ipc/commands";
import { connectionIpQueryKey, queryKeys } from "@/ipc/query-keys";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";

/**
 * The exit address of the running connection. The query key follows the
 * connection itself (node and process), so a result never outlives the
 * connection it describes; the lookup runs on connect only when the user
 * asked for it in settings.
 */
export function ExitIpMetric({ t }: { t: TranslationFunction }) {
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
    gcTime: 0,
    queryFn: () => checkConnectionIp(),
    queryKey: connectionIpQueryKey(connectionKey),
    retry: false,
    staleTime: Number.POSITIVE_INFINITY,
  });
  const checking = ipQuery.fetchStatus === "fetching";
  const result = ipQuery.data;

  let value = "—";
  if (checking) {
    value = t("home.checkIpChecking");
  } else if (ipQuery.isError) {
    value = t("home.checkIpFailed");
  } else if (result) {
    value = [result.ip ?? t("home.checkIpUnknown"), result.countryCode]
      .filter(Boolean)
      .join(" · ");
  }

  return (
    <div>
      <dt>{t("home.exitIp")}</dt>
      <dd className="flex items-center gap-1" data-testid="home-exit-ip">
        {ipQuery.isError && !checking ? (
          // A failed check is a dead end unless the words themselves retry.
          <button
            className="truncate text-start underline-offset-2 hover:underline"
            onClick={() => void ipQuery.refetch()}
            type="button"
          >
            {t("home.checkIpRetry")}
          </button>
        ) : (
          <span className="truncate">{value}</span>
        )}
        <DisabledReason
          reason={connectionKey === null ? t("home.checkIpNeedsConnection") : undefined}
        >
        <Button
          aria-label={t("home.checkIp")}
          className="size-6"
          disabled={connectionKey === null || checking}
          onClick={() => void ipQuery.refetch()}
          size="icon"
          title={t("home.checkIp")}
          type="button"
          variant="ghost"
        >
          {checking ? (
            <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
          ) : (
            <RefreshCw aria-hidden="true" className="size-3.5" />
          )}
        </Button>
        </DisabledReason>
      </dd>
    </div>
  );
}
