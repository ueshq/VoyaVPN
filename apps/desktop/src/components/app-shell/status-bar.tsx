import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Activity, ClipboardCopy, Gauge, LoaderCircle, Power, Settings, WifiOff } from "lucide-react";

import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@voya/ui/components/tooltip";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
import { listProfiles, runtimeStatus, tunProviderDiagnostics, useRuntimeEventStore } from "@/ipc";
import type { CoreState, CoreStateEvent, RuntimeStatusResponse, TunProviderDiagnostics } from "@/ipc/bindings";
import { getErrorMessage } from "@voya/utils/error";
import { formatBytesPerSecond } from "@voya/utils/formatting";
import { useMountedRef } from "@voya/utils/use-mounted-ref";
import { shellTabRoutes, useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";

const PROFILES_QUERY_KEY = ["profiles", { filter: "" }] as const;
const CORE_STATE_TRANSLATION_KEYS = {
  connected: "status.connected",
  connecting: "status.connecting",
  disconnected: "status.disconnected",
  disconnecting: "status.disconnecting",
} as const satisfies Record<CoreState, TranslationKey>;

export function StatusBar() {
  const { t } = useI18n();
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const setCoreState = useRuntimeEventStore((state) => state.setCoreState);
  const statistics = useRuntimeEventStore((state) => state.statistics);
  const activeTab = useShellStore((state) => state.activeTab);
  const pushToast = useToastStore((state) => state.pushToast);
  const initialStatusGenerationRef = useRef(0);
  const mountedRef = useMountedRef();
  const [copyingTunDiagnostics, setCopyingTunDiagnostics] = useState(false);
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
  const stateLabel = t(CORE_STATE_TRANSLATION_KEYS[state]);
  const pidLabel = coreState?.mainPid ? `PID ${coreState.mainPid}` : t("status.noPid");
  const uploadLabel = t("status.upload", { speed: formatBytesPerSecond(statistics?.uploadBytesPerSecond ?? 0) });
  const downloadLabel = t("status.download", { speed: formatBytesPerSecond(statistics?.downloadBytesPerSecond ?? 0) });
  const profilesLabel = t("status.profiles", { count: profilesQuery.data?.length ?? 0 });
  const routeLabel = t("status.route", { route: shellTabRoutes[activeTab] });
  const copyTunDiagnosticsLabel = t("status.copyTunDiagnostics");
  const settingsLabel = t("actions.settings");

  async function copyTunDiagnostics() {
    if (copyingTunDiagnostics) {
      return;
    }

    setCopyingTunDiagnostics(true);
    try {
      if (typeof navigator === "undefined" || !navigator.clipboard?.writeText) {
        throw new Error(t("status.copyTunDiagnosticsClipboardUnavailable"));
      }

      const diagnostics = await tunProviderDiagnostics();
      await navigator.clipboard.writeText(formatTunDiagnosticsForClipboard(diagnostics));
      pushToast({
        description: t("status.copyTunDiagnosticsCopied"),
        severity: "info",
        title: copyTunDiagnosticsLabel,
      });
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("status.copyTunDiagnosticsFailed"),
      });
    } finally {
      if (mountedRef.current) {
        setCopyingTunDiagnostics(false);
      }
    }
  }

  function openSettings() {
    useShellStore.getState().requestTab("settings");
  }

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
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              aria-label={copyTunDiagnosticsLabel}
              className="size-6 text-muted-foreground hover:text-foreground"
              disabled={copyingTunDiagnostics}
              onClick={() => void copyTunDiagnostics()}
              size="icon-xs"
              title={copyTunDiagnosticsLabel}
              type="button"
              variant="ghost"
            >
              {copyingTunDiagnostics ? (
                <LoaderCircle className="size-3.5 animate-spin" aria-hidden="true" />
              ) : (
                <ClipboardCopy className="size-3.5" aria-hidden="true" />
              )}
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top">{copyTunDiagnosticsLabel}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              aria-label={settingsLabel}
              className="size-6 text-muted-foreground hover:text-foreground"
              onClick={openSettings}
              size="icon-xs"
              title={settingsLabel}
              type="button"
              variant="ghost"
            >
              <Settings className="size-3.5" aria-hidden="true" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top">{settingsLabel}</TooltipContent>
        </Tooltip>
      </div>
    </footer>
  );
}

function formatTunDiagnosticsForClipboard(diagnostics: TunProviderDiagnostics) {
  return JSON.stringify(
    {
      type: "voya.tunProviderDiagnostics",
      backend: diagnostics.backend,
      packagingMode: diagnostics.packagingMode,
      systemExtensionState: diagnostics.systemExtensionState,
      status: {
        state: diagnostics.statusState,
        lastError: diagnostics.lastError,
        message: diagnostics.message,
      },
      paths: {
        container: diagnostics.containerPath,
        status: diagnostics.statusPath,
        log: diagnostics.logPath,
        providerBundle: diagnostics.providerBundlePath,
        expectedProvider: diagnostics.expectedProviderPath,
      },
      registrationPaths: diagnostics.registrationPaths,
      breadcrumbs: diagnostics.breadcrumbs,
      providerLogTail: diagnostics.providerLogTail,
      hostLogTail: diagnostics.hostLogTail,
    },
    null,
    2,
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
