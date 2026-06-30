import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { Activity, Gauge, LoaderCircle, Power, WifiOff } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { useI18n } from "@/i18n/use-i18n";
import { listProfiles, runtimeStatus, useRuntimeEventStore } from "@/ipc";
import type { CoreStateEvent, RuntimeStatusResponse } from "@/ipc/bindings";
import { formatBytesPerSecond } from "@/lib/formatting";
import { useMountedRef } from "@/lib/use-mounted-ref";
import { shellTabRoutes, useShellStore } from "@/stores/shell-store";

const PROFILES_QUERY_KEY = ["profiles", { filter: "" }] as const;

export function StatusBar() {
  const { t } = useI18n();
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const setCoreState = useRuntimeEventStore((state) => state.setCoreState);
  const statistics = useRuntimeEventStore((state) => state.statistics);
  const activeTab = useShellStore((state) => state.activeTab);
  const initialStatusGenerationRef = useRef(0);
  const mountedRef = useMountedRef();
  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, null),
    queryKey: PROFILES_QUERY_KEY,
  });

  useEffect(() => {
    const generation = ++initialStatusGenerationRef.current;
    const isCurrent = () => mountedRef.current && generation === initialStatusGenerationRef.current;

    void runtimeStatus()
      .then((status) => {
        if (isCurrent()) {
          setCoreState(statusToCoreState(status));
        }
      })
      .catch(() => undefined);

    return () => {
      initialStatusGenerationRef.current += 1;
    };
  }, [mountedRef, setCoreState]);

  const state = coreState?.state ?? "disconnected";
  const StateIcon = state === "connected" ? Power : state === "disconnected" ? WifiOff : LoaderCircle;
  const stateLabel = t(`status.${state}`);
  const pidLabel = coreState?.mainPid ? `PID ${coreState.mainPid}` : t("status.noPid");
  const uploadLabel = t("status.upload", { speed: formatBytesPerSecond(statistics?.uploadBytesPerSecond ?? 0) });
  const downloadLabel = t("status.download", { speed: formatBytesPerSecond(statistics?.downloadBytesPerSecond ?? 0) });
  const profilesLabel = t("status.profiles", { count: profilesQuery.data?.length ?? 0 });
  const routeLabel = t("status.route", { route: shellTabRoutes[activeTab] });

  return (
    <footer
      aria-label={t("status.aria")}
      className="flex h-8 min-w-0 shrink-0 items-center gap-2 overflow-hidden border-t border-border bg-sidebar px-2 text-xs text-muted-foreground"
      data-testid="status-bar"
    >
      <div className="flex min-w-0 shrink-0 items-center gap-2 font-medium text-foreground">
        <StateIcon
          className={state === "connecting" || state === "disconnecting" ? "size-3.5 animate-spin" : "size-3.5"}
          aria-hidden="true"
        />
        <span className="truncate">{stateLabel}</span>
      </div>
      <Badge
        className="h-5 max-w-40 min-w-0 shrink justify-start bg-background px-2 text-subtle"
        title={routeLabel}
        variant="outline"
      >
        <span className="min-w-0 truncate">{routeLabel}</span>
      </Badge>
      <div className="hidden min-w-0 items-center gap-1.5 md:flex">
        <Badge
          className="h-5 max-w-24 justify-start bg-background px-2 text-subtle"
          title={pidLabel}
          variant="outline"
        >
          <span className="min-w-0 truncate">{pidLabel}</span>
        </Badge>
      </div>
      <div className="ms-auto flex min-w-0 items-center gap-2">
        <Badge
          className="hidden h-5 w-24 min-w-0 shrink justify-start bg-background px-2 text-subtle sm:inline-flex"
          title={profilesLabel}
          variant="outline"
        >
          <span className="min-w-0 truncate">{profilesLabel}</span>
        </Badge>
        <Badge
          className="hidden h-5 w-28 min-w-0 shrink justify-start bg-background px-2 text-subtle lg:inline-flex"
          title={uploadLabel}
          variant="outline"
        >
          <Activity className="size-3.5" aria-hidden="true" />
          <span className="min-w-0 truncate font-mono tabular-nums">{uploadLabel}</span>
        </Badge>
        <Badge
          className="h-5 w-28 min-w-0 shrink justify-start bg-background px-2 text-subtle"
          title={downloadLabel}
          variant="outline"
        >
          <Gauge className="size-3.5" aria-hidden="true" />
          <span className="min-w-0 truncate font-mono tabular-nums">{downloadLabel}</span>
        </Badge>
      </div>
    </footer>
  );
}

function statusToCoreState(status: RuntimeStatusResponse): CoreStateEvent {
  return {
    activeProfileId: status.activeProfileId,
    mainPid: status.mainPid,
    prePid: status.prePid,
    runningCoreType: status.runningCoreType,
    state: status.state,
  };
}
