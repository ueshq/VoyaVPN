import { useEffect, useState } from "react";
import {
  Activity,
  Gauge,
  KeyRound,
  LoaderCircle,
  Plug,
  Power,
  PowerOff,
  RotateCw,
  Shield,
  WifiOff,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useI18n } from "@/i18n/use-i18n";
import {
  connectActiveProfile,
  disconnectCore,
  IpcCommandError,
  restartCore,
  runtimeStatus,
  setTunEnabled,
  setSystemProxyMode,
  systemProxyStatus,
  sudoBeginCollection,
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
import { cn } from "@/lib/utils";
import { useModalStore } from "@/stores/modal-store";

const SYS_PROXY_TYPE = {
  forcedClear: 0,
  forcedChange: 1,
  unchanged: 2,
  pac: 3,
} as const satisfies Record<SysProxyMode, number>;

export function StatusBar() {
  const { t } = useI18n();
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const setCoreState = useRuntimeEventStore((state) => state.setCoreState);
  const statistics = useRuntimeEventStore((state) => state.statistics);
  const sysProxy = useRuntimeEventStore((state) => state.sysProxy);
  const setSysProxy = useRuntimeEventStore((state) => state.setSysProxy);
  const tun = useRuntimeEventStore((state) => state.tun);
  const setTun = useRuntimeEventStore((state) => state.setTun);
  const openModal = useModalStore((state) => state.openModal);
  const [pendingAction, setPendingAction] = useState<"connect" | "disconnect" | "restart" | "tun" | null>(null);

  useEffect(() => {
    let disposed = false;

    void runtimeStatus()
      .then((status) => {
        if (!disposed) {
          setCoreState(statusToCoreState(status));
        }
      })
      .catch(() => undefined);
    void systemProxyStatus()
      .then((status) => {
        if (!disposed) {
          setSysProxy(statusToSysProxyChanged(status));
        }
      })
      .catch(() => undefined);
    void tunStatus()
      .then((status) => {
        if (!disposed) {
          setTun(statusToTunChanged(status));
        }
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
    };
  }, [setCoreState, setSysProxy, setTun]);

  const state = coreState?.state ?? "disconnected";
  const connected = state === "connected";
  const busy = state === "connecting" || state === "disconnecting" || pendingAction !== null;
  const StateIcon = state === "connected" ? Power : state === "disconnected" ? WifiOff : LoaderCircle;
  const stateLabel = t(`status.${state}`);
  const coreLabel = coreState?.runningCoreType ? formatCoreType(coreState.runningCoreType) : t("status.noCore");
  const pidLabel = coreState?.mainPid ? `PID ${coreState.mainPid}` : t("status.noPid");
  const requestedProxyMode = sysProxy?.requestedMode ?? "forcedClear";
  const effectiveProxyLabel = formatSysProxy(sysProxy?.effectiveMode, t);
  const tunEnabled = tun?.enabled ?? false;
  const tunActionLabel = tunEnabled ? t("actions.disableTun") : t("actions.enableTun");
  const tunStateLabel = tunEnabled ? t("status.tunOn") : t("status.tunOff");
  const uploadLabel = t("status.upload", { speed: formatBytesPerSecond(statistics?.uploadBytesPerSecond ?? 0) });
  const downloadLabel = t("status.download", { speed: formatBytesPerSecond(statistics?.downloadBytesPerSecond ?? 0) });
  const profilesLabel = t("status.profiles", { count: 0 });

  async function runRuntimeAction(action: "connect" | "disconnect" | "restart") {
    setPendingAction(action);
    try {
      const status =
        action === "connect"
          ? await connectActiveProfile()
          : action === "disconnect"
            ? await disconnectCore()
            : await restartCore();

      setCoreState(statusToCoreState(status));
    } catch (error) {
      if (shouldOpenSudoPrompt(error)) {
        openModal("sudo");
      }
    } finally {
      setPendingAction(null);
    }
  }

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
        const current = await tunStatus();
        if (current.requiresSudoPassword && !current.sudoPasswordPresent) {
          const collection = await sudoBeginCollection();
          if (collection.state === "required") {
            openModal("sudo", { intent: "enableTun" });
            return;
          }
        }
      }

      const status = await setTunEnabled(nextEnabled);
      setTun(statusToTunChanged(status));
    } catch (error) {
      if (shouldOpenSudoPrompt(error)) {
        openModal("sudo", { intent: "enableTun" });
      }
    } finally {
      setPendingAction(null);
    }
  }

  return (
    <footer
      aria-label={t("status.aria")}
      className="flex h-11 min-w-0 shrink-0 items-center gap-3 overflow-hidden border-t border-border bg-card px-4 text-xs text-muted-foreground"
      data-testid="status-bar"
    >
      <div className="flex min-w-0 shrink-0 items-center gap-2 font-medium text-foreground">
        <StateIcon
          className={state === "connecting" || state === "disconnecting" ? "size-3.5 animate-spin" : "size-3.5"}
          aria-hidden="true"
        />
        <span className="truncate">{stateLabel}</span>
      </div>
      <div className="hidden min-w-0 items-center gap-1.5 md:flex">
        <Badge
          className="h-6 max-w-28 justify-start bg-background px-2 text-muted-foreground"
          title={coreLabel}
          variant="outline"
        >
          <span className="min-w-0 truncate">{coreLabel}</span>
        </Badge>
        <Badge
          className="h-6 max-w-24 justify-start bg-background px-2 text-muted-foreground"
          title={pidLabel}
          variant="outline"
        >
          <span className="min-w-0 truncate">{pidLabel}</span>
        </Badge>
      </div>
      <Separator orientation="vertical" className="h-4" />
      <div className="flex items-center gap-1">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              aria-label={t("actions.connect")}
              className="size-7"
              disabled={busy || connected}
              onClick={() => void runRuntimeAction("connect")}
              size="icon"
              title={t("actions.connect")}
              type="button"
              variant={connected ? "secondary" : "outline"}
            >
              <Power className="size-3.5" aria-hidden="true" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top">{t("actions.connect")}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              aria-label={t("actions.disconnect")}
              className="size-7"
              disabled={busy || !connected}
              onClick={() => void runRuntimeAction("disconnect")}
              size="icon"
              title={t("actions.disconnect")}
              type="button"
              variant="outline"
            >
              <PowerOff className="size-3.5" aria-hidden="true" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top">{t("actions.disconnect")}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              aria-label={t("actions.restart")}
              className="size-7"
              disabled={busy || !connected}
              onClick={() => void runRuntimeAction("restart")}
              size="icon"
              title={t("actions.restart")}
              type="button"
              variant="outline"
            >
              <RotateCw className="size-3.5" aria-hidden="true" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top">{t("actions.restart")}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              aria-label={t("actions.sudo")}
              className="size-7"
              onClick={() => openModal("sudo")}
              size="icon"
              title={t("actions.sudo")}
              type="button"
              variant="ghost"
            >
              <KeyRound className="size-3.5" aria-hidden="true" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top">{t("actions.sudo")}</TooltipContent>
        </Tooltip>
      </div>
      <Separator orientation="vertical" className="h-4" />
      <div className="hidden min-w-0 items-center gap-2 md:flex">
        <Shield className="size-3.5" aria-hidden="true" />
        <div
          className="flex h-7 shrink-0 items-center rounded-md bg-muted p-[2px]"
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
                      "h-[22px] w-12 rounded-sm px-1 text-[11px] leading-none shadow-none focus-visible:relative focus-visible:z-10",
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
          className="hidden h-6 max-w-44 justify-start bg-background px-2 text-muted-foreground lg:inline-flex"
          title={effectiveProxyLabel}
          variant="outline"
        >
          <span className="min-w-0 truncate">{effectiveProxyLabel}</span>
        </Badge>
      </div>
      <Separator orientation="vertical" className="hidden h-4 md:block" />
      <div className="flex min-w-0 shrink-0 items-center gap-2">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              aria-label={tunActionLabel}
              aria-pressed={tunEnabled}
              className="size-7"
              disabled={pendingAction === "tun"}
              onClick={() => void runTunToggle()}
              size="icon"
              title={tunActionLabel}
              type="button"
              variant={tunEnabled ? "secondary" : "outline"}
            >
              <Plug className="size-3.5" aria-hidden="true" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top">{tunActionLabel}</TooltipContent>
        </Tooltip>
        <Badge
          className="hidden h-6 max-w-20 justify-start bg-background px-2 text-muted-foreground sm:inline-flex"
          title={tunStateLabel}
          variant="outline"
        >
          <span className="min-w-0 truncate">{tunStateLabel}</span>
        </Badge>
      </div>
      <div className="ms-auto flex min-w-0 items-center gap-2">
        <Badge
          className="hidden h-6 w-24 min-w-0 shrink justify-start bg-background px-2 text-muted-foreground sm:inline-flex"
          title={profilesLabel}
          variant="outline"
        >
          <span className="min-w-0 truncate">{profilesLabel}</span>
        </Badge>
        <Badge
          className="hidden h-6 w-28 min-w-0 shrink justify-start bg-background px-2 text-muted-foreground lg:inline-flex"
          title={uploadLabel}
          variant="outline"
        >
          <Activity className="size-3.5" aria-hidden="true" />
          <span className="min-w-0 truncate">{uploadLabel}</span>
        </Badge>
        <Badge
          className="h-6 w-28 min-w-0 shrink justify-start bg-background px-2 text-muted-foreground"
          title={downloadLabel}
          variant="outline"
        >
          <Gauge className="size-3.5" aria-hidden="true" />
          <span className="min-w-0 truncate">{downloadLabel}</span>
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

function formatCoreType(coreType: number) {
  switch (coreType) {
    case 2:
      return "Xray";
    case 24:
      return "sing-box";
    case 13:
      return "mihomo";
    default:
      return `Core ${coreType}`;
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

function formatBytesPerSecond(value: number) {
  if (value < 1024) {
    return `${Math.round(value)} B/s`;
  }

  const units = ["KB/s", "MB/s", "GB/s"];
  let scaled = value / 1024;
  let unitIndex = 0;

  while (scaled >= 1024 && unitIndex < units.length - 1) {
    scaled /= 1024;
    unitIndex += 1;
  }

  return `${scaled >= 10 ? scaled.toFixed(0) : scaled.toFixed(1)} ${units[unitIndex]}`;
}

function shouldOpenSudoPrompt(error: unknown) {
  if (!(error instanceof IpcCommandError)) {
    return false;
  }

  return error.appError.kind === "sudo" || error.message.toLowerCase().includes("sudo password");
}
