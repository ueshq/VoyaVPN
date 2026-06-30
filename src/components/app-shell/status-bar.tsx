import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Activity, Gauge, LoaderCircle, MoreHorizontal, Plug, Power, Shield, WifiOff } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Menubar,
  MenubarCheckboxItem,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarRadioGroup,
  MenubarRadioItem,
  MenubarSeparator,
  MenubarTrigger,
} from "@/components/ui/menubar";
import { Separator } from "@/components/ui/separator";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useI18n } from "@/i18n/use-i18n";
import {
  listProfiles,
  runtimeStatus,
  setTunEnabled,
  setSystemProxyMode,
  systemProxyStatus,
  tunRequestElevation,
  tunStatus,
  useRuntimeEventStore,
} from "@/ipc";
import type {
  CoreStateEvent,
  RuntimeStatusResponse,
  SysProxyChanged,
  SysProxyMode,
  SystemProxyStatusResponse,
  TunChanged,
  TunStatus,
} from "@/ipc/bindings";
import { CORE_TYPES, formatCoreType } from "@/lib/core-types";
import { formatBytesPerSecond } from "@/lib/formatting";
import { useMountedRef } from "@/lib/use-mounted-ref";
import { cn, getErrorMessage } from "@/lib/utils";
import { shellTabRoutes, useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";

const SYS_PROXY_TYPE = {
  forcedClear: 0,
  forcedChange: 1,
  unchanged: 2,
  pac: 3,
} as const satisfies Record<SysProxyMode, number>;

const PROFILES_QUERY_KEY = ["profiles", { filter: "" }] as const;

export function StatusBar() {
  const { t } = useI18n();
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const setCoreState = useRuntimeEventStore((state) => state.setCoreState);
  const statistics = useRuntimeEventStore((state) => state.statistics);
  const sysProxy = useRuntimeEventStore((state) => state.sysProxy);
  const setSysProxy = useRuntimeEventStore((state) => state.setSysProxy);
  const tun = useRuntimeEventStore((state) => state.tun);
  const setTun = useRuntimeEventStore((state) => state.setTun);
  const pushToast = useToastStore((state) => state.pushToast);
  const activeTab = useShellStore((state) => state.activeTab);
  const [pendingAction, setPendingAction] = useState<"tun" | null>(null);
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
    void systemProxyStatus()
      .then((status) => {
        if (isCurrent()) {
          setSysProxy(statusToSysProxyChanged(status));
        }
      })
      .catch(() => undefined);
    void tunStatus()
      .then((status) => {
        if (isCurrent()) {
          setTun(statusToTunChanged(status));
        }
      })
      .catch(() => undefined);

    return () => {
      initialStatusGenerationRef.current += 1;
    };
  }, [mountedRef, setCoreState, setSysProxy, setTun]);

  const state = coreState?.state ?? "disconnected";
  const activeProfile = profilesQuery.data?.find((item) => item?.isActive) ?? null;
  const runningCoreType = coreState?.runningCoreType ?? null;
  const displayedCoreType = runningCoreType ?? (activeProfile ? CORE_TYPES.singBox : null);
  const StateIcon = state === "connected" ? Power : state === "disconnected" ? WifiOff : LoaderCircle;
  const stateLabel = t(`status.${state}`);
  const coreLabel = displayedCoreType ? formatCoreType(displayedCoreType) : t("status.noActiveProfile");
  const coreTitle = activeProfile ? coreLabel : t("status.noActiveProfile");
  const pidLabel = coreState?.mainPid ? `PID ${coreState.mainPid}` : t("status.noPid");
  const requestedProxyMode = sysProxy?.requestedMode ?? "forcedClear";
  const effectiveProxyLabel = formatSysProxy(sysProxy?.effectiveMode, t);
  const tunEnabled = tun?.enabled ?? false;
  const tunActionLabel = tunEnabled ? t("actions.disableTun") : t("actions.enableTun");
  const tunStateLabel = tunEnabled ? t("status.tunOn") : t("status.tunOff");
  const uploadLabel = t("status.upload", { speed: formatBytesPerSecond(statistics?.uploadBytesPerSecond ?? 0) });
  const downloadLabel = t("status.download", { speed: formatBytesPerSecond(statistics?.downloadBytesPerSecond ?? 0) });
  const profilesLabel = t("status.profiles", { count: profilesQuery.data?.length ?? 0 });
  const routeLabel = t("status.route", { route: shellTabRoutes[activeTab] });

  async function runProxyMode(mode: SysProxyMode) {
    try {
      const status = await setSystemProxyMode(SYS_PROXY_TYPE[mode]);
      setSysProxy(statusToSysProxyChanged(status));
    } catch {
      return;
    }
  }

  async function runTunToggle() {
    const nextEnabled = !(tun?.enabled ?? false);

    setPendingAction("tun");
    try {
      if (nextEnabled) {
        // Obtain system authorization on demand (one native prompt, no stored
        // password) before switching TUN on.
        const current = await tunStatus();
        if (current.requiresElevation && !current.elevationGranted) {
          const granted = await tunRequestElevation();
          if (!granted.elevationGranted) {
            // User cancelled the native dialog — leave TUN off.
            return;
          }
        }
      }

      const status = await setTunEnabled(nextEnabled);
      setTun(statusToTunChanged(status));
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        title: t(nextEnabled ? "status.tunEnableFailed" : "status.tunDisableFailed"),
      });
    } finally {
      setPendingAction(null);
    }
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
          className="h-5 max-w-28 justify-start bg-background px-2 text-subtle"
          title={coreTitle}
          variant="outline"
        >
          <span className="min-w-0 truncate">{coreLabel}</span>
        </Badge>
        <Badge
          className="h-5 max-w-24 justify-start bg-background px-2 text-subtle"
          title={pidLabel}
          variant="outline"
        >
          <span className="min-w-0 truncate">{pidLabel}</span>
        </Badge>
      </div>
      <Separator orientation="vertical" className="h-3.5" />
      <div className="hidden min-w-0 items-center gap-2 md:flex">
        <Shield className="size-3.5" aria-hidden="true" />
        <div
          className="flex h-6 shrink-0 items-center rounded-md bg-muted p-0.5"
          role="group"
          aria-label={t("status.sysProxyMode")}
        >
          {proxyModeOptions(sysProxy?.pacAvailable ?? false).map((mode) => {
            const selected = requestedProxyMode === mode;
            const modeLabel = formatSysProxy(mode, t);

            return (
              <Tooltip key={mode}>
                <TooltipTrigger asChild>
                  <Button
                    aria-label={modeLabel}
                    aria-pressed={selected}
                    className={cn(
                      "h-4.5 w-12 rounded-sm px-1 text-[11px] leading-none shadow-none focus-visible:relative focus-visible:z-10",
                      selected
                        ? "bg-background text-foreground hover:bg-background hover:text-foreground"
                        : "text-muted-foreground hover:bg-background/60 hover:text-foreground",
                    )}
                    onClick={() => void runProxyMode(mode)}
                    size="sm"
                    title={modeLabel}
                    type="button"
                    variant="ghost"
                  >
                    <span className="min-w-0 truncate">{shortSysProxy(mode, t)}</span>
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="top">{modeLabel}</TooltipContent>
              </Tooltip>
            );
          })}
        </div>
        <Badge
          className="hidden h-5 max-w-44 justify-start bg-background px-2 text-subtle lg:inline-flex"
          title={effectiveProxyLabel}
          variant="outline"
        >
          <span className="min-w-0 truncate">{effectiveProxyLabel}</span>
        </Badge>
      </div>
      <Separator orientation="vertical" className="hidden h-3.5 md:block" />
      <div className="hidden min-w-0 shrink-0 items-center md:flex">
        {/* Single TUN control: shows the on/off state and toggles it. Enabling
            requests system authorization on demand (no stored password). */}
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              aria-label={tunActionLabel}
              aria-pressed={tunEnabled}
              className="h-6 gap-1.5 px-2"
              disabled={pendingAction === "tun"}
              onClick={() => void runTunToggle()}
              size="sm"
              title={tunActionLabel}
              type="button"
              variant={tunEnabled ? "secondary" : "outline"}
            >
              {pendingAction === "tun" ? (
                <LoaderCircle className="size-3.5 animate-spin" aria-hidden="true" />
              ) : (
                <Plug className="size-3.5" aria-hidden="true" />
              )}
              <span className="min-w-0 truncate">{tunStateLabel}</span>
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top">{tunActionLabel}</TooltipContent>
        </Tooltip>
      </div>
      {/* Below md: the core info, proxy mode, and TUN controls above are hidden;
          surface them here so small windows keep access to every key control. */}
      <Menubar className="h-7 shrink-0 border-0 bg-transparent p-0 shadow-none md:hidden">
        <MenubarMenu>
          <MenubarTrigger
            aria-label={t("status.moreControls")}
            className="size-7 justify-center rounded-md p-0 text-muted-foreground"
            title={t("status.moreControls")}
          >
            <MoreHorizontal className="size-3.5" aria-hidden="true" />
          </MenubarTrigger>
          <MenubarContent align="start">
            <MenubarItem disabled>{coreLabel}</MenubarItem>
            <MenubarItem disabled>{pidLabel}</MenubarItem>
            <MenubarSeparator />
            <MenubarRadioGroup
              onValueChange={(value) => void runProxyMode(value as SysProxyMode)}
              value={requestedProxyMode}
            >
              {proxyModeOptions(sysProxy?.pacAvailable ?? false).map((mode) => (
                <MenubarRadioItem key={mode} value={mode}>
                  {formatSysProxy(mode, t)}
                </MenubarRadioItem>
              ))}
            </MenubarRadioGroup>
            <MenubarItem disabled>{effectiveProxyLabel}</MenubarItem>
            <MenubarSeparator />
            <MenubarCheckboxItem
              checked={tunEnabled}
              disabled={pendingAction === "tun"}
              onCheckedChange={() => void runTunToggle()}
            >
              {t("status.tun")}
            </MenubarCheckboxItem>
          </MenubarContent>
        </MenubarMenu>
      </Menubar>
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

function statusToSysProxyChanged(status: SystemProxyStatusResponse): SysProxyChanged {
  return {
    effectiveMode: sysProxyTypeToMode(status.effectiveMode),
    pacAvailable: status.pacAvailable,
    proxy: status.proxy,
    requestedMode: sysProxyTypeToMode(status.requestedMode),
  };
}

function statusToTunChanged(status: TunStatus): TunChanged {
  return {
    enabled: status.enabled,
  };
}

function sysProxyTypeToMode(mode: number): SysProxyMode {
  switch (mode) {
    case SYS_PROXY_TYPE.forcedChange:
      return "forcedChange";
    case SYS_PROXY_TYPE.unchanged:
      return "unchanged";
    case SYS_PROXY_TYPE.pac:
      return "pac";
    case SYS_PROXY_TYPE.forcedClear:
    default:
      return "forcedClear";
  }
}

function proxyModeOptions(pacAvailable: boolean): SysProxyMode[] {
  return pacAvailable
    ? ["forcedClear", "forcedChange", "unchanged", "pac"]
    : ["forcedClear", "forcedChange", "unchanged"];
}

function shortSysProxy(mode: SysProxyMode, t: ReturnType<typeof useI18n>["t"]) {
  switch (mode) {
    case "forcedChange":
      return t("status.sysProxySetShort");
    case "forcedClear":
      return t("status.sysProxyClearShort");
    case "pac":
      return t("status.sysProxyPacShort");
    case "unchanged":
    default:
      return t("status.sysProxyKeepShort");
  }
}

function formatSysProxy(mode: SysProxyMode | undefined, t: ReturnType<typeof useI18n>["t"]) {
  switch (mode) {
    case "forcedChange":
      return t("status.sysProxyForced");
    case "forcedClear":
      return t("status.sysProxyCleared");
    case "pac":
      return t("status.sysProxyPac");
    case "unchanged":
    default:
      return t("status.sysProxyUnchanged");
  }
}
