import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { LoaderCircle, RotateCw, ShieldCheck, ShieldOff } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { Card, CardContent, CardHeader, CardTitle } from "@voya/ui/components/card";
import { Label } from "@voya/ui/components/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@voya/ui/components/select";
import { Separator } from "@voya/ui/components/separator";
import { Switch } from "@voya/ui/components/switch";
import { PageTitle } from "@/components/app-shell/page-section";
import { useI18n } from "@voya/i18n/use-i18n";
import type { Locale } from "@voya/i18n";
import { getVersion } from "@/ipc/updater";
import { persistLanguage } from "@/features/settings/use-app-settings";
import {
  connectActiveProfile,
  disconnectCore,
  listProfiles,
  restartCore,
  runtimeStatus,
  setActiveProfile,
  setSystemProxyMode,
  setTunEnabled,
  systemProxyStatus,
  tunRequestElevation,
  tunStatus,
  useRuntimeEventStore,
} from "@/ipc";
import type { SysProxyMode, TunChanged, TunStatus } from "@/ipc/bindings";
import { getErrorMessage } from "@voya/utils/error";
import { cn } from "@voya/ui/lib/utils";
import { useModalStore } from "@/stores/modal-store";
import { useToastStore } from "@/stores/toast-store";

import { NodeList } from "./node-list";
import {
  missingCorePayload,
  PROXY_MODE_OPTIONS,
  runWithElevation,
  statusToCoreState,
  statusToSysProxyChanged,
  statusToTunChanged,
  SYS_PROXY_TYPE,
} from "./runtime-action";

type RuntimeAction = "connect" | "disconnect" | "restart";
type Translation = ReturnType<typeof useI18n>["t"];

/**
 * Connection home — the default view: the app-name page title, a Quick Actions
 * card (connection status header + connect toggle, system proxy, TUN, language,
 * version rows), and the node list card. Single-accent discipline: interactive
 * chrome stays brand blue; affirmative green (`--connected` / `--connected-glow`)
 * is reserved for the achieved protected state (status disc + headline). It only
 * reuses the existing runtime actions and {@link useRuntimeEventStore}; no new
 * IPC is introduced. Decorative motion (the status-light spinner) inherits the
 * global `prefers-reduced-motion` guard in globals.css.
 */
export function HomeScreen() {
  const { language, localeOptions, setLocale, t } = useI18n();
  const home = useHomeRuntime(t);
  const queryClient = useQueryClient();
  const pushToast = useToastStore((state) => state.pushToast);
  // Serialize language persists at the UI level: the select is disabled while a
  // write is in flight so rapid switches cannot interleave the read-modify-write.
  const [languageSaving, setLanguageSaving] = useState(false);
  const versionQuery = useQuery({
    queryFn: () => getVersion(),
    queryKey: ["app-version"],
    staleTime: Infinity,
  });

  const headline = home.connected
    ? t("home.protected")
    : home.state === "connecting"
      ? t("status.connecting")
      : home.state === "disconnecting"
        ? t("status.disconnecting")
        : t("home.unprotected");
  const hint = home.connected
    ? t("home.protectedHint")
    : home.state === "disconnected"
      ? t("home.unprotectedHint")
      : "";
  const pidLabel = home.mainPid ? `PID ${home.mainPid}` : t("status.noPid");

  async function changeLanguage(code: Locale) {
    if (code === language || languageSaving) {
      return;
    }
    // Apply instantly (i18next + localStorage), then persist to the settings DB
    // so the choice survives a cold start. On persist failure the UI keeps the
    // switched language and a toast explains what did not stick.
    setLanguageSaving(true);
    await setLocale(code);
    try {
      await persistLanguage(queryClient, code);
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("status.languageSaveFailed"),
      });
    } finally {
      setLanguageSaving(false);
    }
  }

  return (
    <section
      aria-label={t("home.aria")}
      className="flex h-full min-h-0 flex-col overflow-y-auto"
      data-testid="home-screen"
    >
      <div className="mx-auto flex min-h-0 w-full max-w-2xl flex-1 flex-col px-6 pb-8">
        <PageTitle className="px-0" title={t("app.name")} />

        <Card className="shrink-0 gap-0 py-0">
          <CardHeader className="border-b px-4 py-4" data-testid="home-status-card">
            <ConnectionStatus
              connected={home.connected}
              headline={headline}
              hint={hint}
              inProgress={home.inProgress}
              pidLabel={pidLabel}
            />
          </CardHeader>
          <CardContent className="px-4">
            <ConnectRow
              busy={home.busy}
              checked={home.connected || home.state === "connecting"}
              connected={home.connected}
              onPrimaryAction={home.handlePrimaryAction}
              onRestart={home.restart}
              t={t}
            />
            <Separator />
            <SysProxyRow
              onProxyModeChange={home.changeProxyMode}
              pacAvailable={home.pacAvailable}
              proxyPending={home.proxyPending}
              requestedProxyMode={home.requestedProxyMode}
              t={t}
            />
            <Separator />
            <TunRow
              onTunToggle={home.toggleTun}
              t={t}
              tunEnabled={home.tunEnabled}
              tunPending={home.tunPending}
              tunProviderSummary={home.tunProviderSummary}
            />
            <Separator />
            <div className="flex items-center justify-between gap-3 py-2.5">
              <Label className="text-sm font-medium text-foreground" htmlFor="home-language-select">
                {t("modal.language")}
              </Label>
              <Select disabled={languageSaving} onValueChange={(value) => void changeLanguage(value as Locale)} value={language}>
                <SelectTrigger className="w-40" id="home-language-select">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {localeOptions.map((locale) => (
                    <SelectItem key={locale.code} value={locale.code}>
                      {locale.nativeName}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <Separator />
            <div className="flex items-center justify-between gap-3 py-2.5">
              <span className="text-sm font-medium text-foreground">{t("home.version")}</span>
              <span className="text-sm tabular-nums text-muted-foreground">{versionQuery.data ?? "—"}</span>
            </div>
          </CardContent>
        </Card>

        <Card className="mt-4 flex min-h-0 flex-1 flex-col gap-3 py-4">
          <CardHeader className="px-4">
            <CardTitle className="text-sm font-semibold">{t("home.nodes")}</CardTitle>
          </CardHeader>
          <CardContent className="flex min-h-0 flex-1 flex-col px-4">
            <NodeList
              isPending={home.profilesPending}
              onActivate={home.activateProfile}
              onSelect={home.selectProfile}
              profiles={home.profiles}
              runningId={home.runningId}
              selectedId={home.selectedId}
              switchingId={home.switchingId}
            />
          </CardContent>
        </Card>
      </div>
    </section>
  );
}

function useHomeRuntime(t: Translation) {
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const setCoreState = useRuntimeEventStore((state) => state.setCoreState);
  const sysProxy = useRuntimeEventStore((state) => state.sysProxy);
  const setSysProxy = useRuntimeEventStore((state) => state.setSysProxy);
  const tun = useRuntimeEventStore((state) => state.tun);
  const setTun = useRuntimeEventStore((state) => state.setTun);
  const openModal = useModalStore((state) => state.openModal);
  const pushToast = useToastStore((state) => state.pushToast);
  const queryClient = useQueryClient();
  const [pendingAction, setPendingAction] = useState<RuntimeAction | null>(null);
  const [proxyPending, setProxyPending] = useState<SysProxyMode | null>(null);
  // TUN toggling is tracked separately from connect/disconnect so the two
  // controls never block each other.
  const [tunPending, setTunPending] = useState(false);
  // Local node selection (blue highlight). Seeded from the persisted active
  // profile; single-clicks move it without touching the backend.
  const [selectedId, setSelectedId] = useState<string | null>(null);
  // Tracks the active profile the selection was last seeded from, so re-seeding
  // only fires when the active profile actually changes.
  const [seededFor, setSeededFor] = useState<string | null>(null);
  // The node whose switch+connect is currently in flight (spinner / re-entry guard).
  const [switchingId, setSwitchingId] = useState<string | null>(null);
  // Shares the ProfilesScreen query cache (same key) so resolving the active
  // node's name here costs no extra fetch and stays in sync after a switch.
  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, null),
    queryKey: ["profiles", { filter: "" }],
  });

  // Home owns the system-proxy / TUN controls, so it seeds their live OS state
  // into the store on mount. Transient `sysProxyChanged` / `tunChanged` events
  // keep it fresh afterwards.
  useEffect(() => {
    let cancelled = false;

    void systemProxyStatus()
      .then((status) => {
        if (!cancelled) {
          setSysProxy(statusToSysProxyChanged(status));
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          pushToast({
            description: getErrorMessage(error),
            severity: "error",
            title: t("status.sysProxyStatusFailed"),
          });
        }
      });
    void tunStatus()
      .then((status) => {
        if (!cancelled) {
          setTun(statusToTunChanged(status));
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          pushToast({
            description: getErrorMessage(error),
            severity: "error",
            title: t("status.tunStatusFailed"),
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [pushToast, setSysProxy, setTun, t]);

  const state = coreState?.state ?? "disconnected";
  const connected = state === "connected";
  const inProgress = state === "connecting" || state === "disconnecting";
  const busy = inProgress || pendingAction !== null || switchingId !== null;

  const activeProfile = profilesQuery.data?.find((item) => item.isActive) ?? null;
  const activeProfileId = activeProfile?.profile.id ?? null;
  // The green "live" dot follows the node that is actually running, which differs
  // from the persisted-active node only while disconnected.
  const runningId = connected ? (coreState?.activeProfileId ?? null) : null;
  const requestedProxyMode = sysProxy?.requestedMode ?? "forcedClear";
  const pacAvailable = sysProxy?.pacAvailable ?? false;
  const tunEnabled = tun?.enabled ?? false;
  const tunProviderSummary = tun ? tunProviderLabel(tun, t) : null;

  // Seed the local selection from the persisted active profile and re-sync it
  // whenever the active profile changes (e.g. after a switch). Adjusting state
  // during render (React's documented pattern) instead of in an effect avoids a
  // cascading-render lint and an extra paint. A single-click only moves
  // `selectedId`, not the active profile, so the selection is never clobbered.
  if (activeProfileId && activeProfileId !== seededFor) {
    setSeededFor(activeProfileId);
    setSelectedId(activeProfileId);
  }

  async function runRuntimeAction(action: RuntimeAction) {
    setPendingAction(action);
    try {
      const status = await runWithElevation(() =>
        action === "connect"
          ? connectActiveProfile()
          : action === "disconnect"
            ? disconnectCore()
            : restartCore(),
      );

      setCoreState(statusToCoreState(status));
    } catch (error) {
      const missingCore = missingCorePayload(error);
      if (missingCore) {
        openModal("missingCore", { missingCore });
      } else {
        pushToast({
          description: getErrorMessage(error),
          severity: "error",
          title: runtimeActionLabel(action, t),
        });
      }
      await refreshRuntimeState();
    } finally {
      setPendingAction(null);
    }
  }

  async function refreshRuntimeState() {
    try {
      const status = await runtimeStatus();
      setCoreState(statusToCoreState(status));
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("status.runtimeStatusFailed"),
      });
    }
  }

  // Switch the active profile to `indexId` and apply it: restart the tunnel when
  // already connected, otherwise connect. Drives double-click / Enter and the
  // Connect button when its selection differs from the active profile.
  async function switchActiveAndApply(indexId: string) {
    if (switchingId !== null) {
      return;
    }

    setSelectedId(indexId);
    setSwitchingId(indexId);
    const wasConnected = connected;
    try {
      await setActiveProfile(indexId);
      const status = await runWithElevation(() =>
        wasConnected ? restartCore() : connectActiveProfile(),
      );
      setCoreState(statusToCoreState(status));
    } catch (error) {
      const missingCore = missingCorePayload(error);
      if (missingCore) {
        openModal("missingCore", { missingCore });
      } else {
        pushToast({
          description: getErrorMessage(error),
          severity: "error",
          title: t(wasConnected ? "actions.restart" : "actions.connect"),
        });
        await refreshRuntimeState();
      }
    } finally {
      // The active profile changed in the DB regardless of connect success, so
      // refresh the cache that drives the active-node highlight.
      await queryClient.invalidateQueries({ queryKey: ["profiles"] });
      setSwitchingId(null);
    }
  }

  function handlePrimaryAction() {
    if (connected) {
      void runRuntimeAction("disconnect");

      return;
    }
    // Connect to the locally-selected node. When it differs from the persisted
    // active profile, switch first so connect uses it; otherwise connect directly.
    if (selectedId && selectedId !== activeProfileId) {
      void switchActiveAndApply(selectedId);

      return;
    }
    void runRuntimeAction("connect");
  }

  async function runProxyMode(mode: SysProxyMode) {
    if (proxyPending !== null || (mode === "pac" && !pacAvailable)) {
      return;
    }

    setProxyPending(mode);
    try {
      const status = await setSystemProxyMode(SYS_PROXY_TYPE[mode]);
      setSysProxy(statusToSysProxyChanged(status));
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("status.sysProxyChangeFailed"),
      });
    } finally {
      setProxyPending(null);
    }
  }

  async function runTunToggle() {
    const nextEnabled = !(tun?.enabled ?? false);

    setTunPending(true);
    try {
      if (nextEnabled) {
        // Obtain system authorization on demand (one native prompt, no stored
        // password) before switching TUN on.
        const current = await tunStatus();
        if (current.backend !== "process" && !current.nativeComponentReady) {
          pushToast({
            description: current.lastProviderError ?? t("status.nativeTunnelMissing"),
            severity: "error",
            title: t("status.tunEnableFailed"),
          });
          return;
        }
        if (current.providerPathMismatch) {
          pushToast({
            description: tunProviderPathMismatchDescription(current, t),
            severity: "error",
            title: t("status.tunEnableFailed"),
          });
          return;
        }
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
        severity: "error",
        title: t(nextEnabled ? "status.tunEnableFailed" : "status.tunDisableFailed"),
      });
    } finally {
      setTunPending(false);
    }
  }

  function activateProfile(indexId: string) {
    void switchActiveAndApply(indexId);
  }

  function changeProxyMode(mode: SysProxyMode) {
    void runProxyMode(mode);
  }

  function restart() {
    void runRuntimeAction("restart");
  }

  function selectProfile(indexId: string) {
    setSelectedId(indexId);
  }

  function toggleTun() {
    void runTunToggle();
  }

  return {
    activateProfile,
    busy,
    changeProxyMode,
    connected,
    handlePrimaryAction,
    inProgress,
    mainPid: coreState?.mainPid ?? null,
    pacAvailable,
    profiles: profilesQuery.data ?? [],
    profilesPending: profilesQuery.isPending,
    proxyPending,
    requestedProxyMode,
    restart,
    runningId,
    selectProfile,
    selectedId,
    state,
    switchingId,
    toggleTun,
    tunEnabled,
    tunPending,
    tunProviderSummary,
  };
}

// Compact horizontal status block in the Quick Actions card header. Its muted
// hint doubles as the card's explanatory line; the PID meta keeps the runtime
// detail the removed status bar used to carry.
function ConnectionStatus({
  connected,
  headline,
  hint,
  inProgress,
  pidLabel,
}: {
  connected: boolean;
  headline: string;
  hint: string;
  inProgress: boolean;
  pidLabel: string;
}) {
  const StatusIcon = connected ? ShieldCheck : inProgress ? LoaderCircle : ShieldOff;

  return (
    <div className="flex items-center gap-4">
      <span
        aria-hidden="true"
        className={cn(
          "flex size-12 shrink-0 items-center justify-center rounded-full border transition-colors",
          connected
            ? "border-connected/40 bg-connected/10 text-connected shadow-[var(--connected-glow)]"
            : "border-border bg-surface-sunken text-muted-foreground",
        )}
      >
        <StatusIcon className={cn("size-6", inProgress && "animate-spin")} />
      </span>
      <div className="min-w-0">
        <p
          className={cn(
            "font-display text-lg font-semibold tracking-tight",
            connected ? "text-connected" : "text-foreground",
          )}
        >
          {headline}
        </p>
        {hint ? <p className="mt-0.5 text-sm text-muted-foreground">{hint}</p> : null}
        {connected ? <p className="mt-0.5 text-xs tabular-nums text-subtlest">{pidLabel}</p> : null}
      </div>
    </div>
  );
}

// The reference design's toggle: the switch is a pure presentation swap over the
// toggle-shaped `handlePrimaryAction` (connected → disconnect; a diverged local
// selection → switch-and-connect; otherwise connect). The accessible name stays
// a constant "Connect"; state travels via `aria-checked`.
function ConnectRow({
  busy,
  checked,
  connected,
  onPrimaryAction,
  onRestart,
  t,
}: {
  busy: boolean;
  checked: boolean;
  connected: boolean;
  onPrimaryAction: () => void;
  onRestart: () => void;
  t: Translation;
}) {
  return (
    <div className="flex items-center justify-between gap-3 py-2.5">
      <Label className="text-sm font-medium text-foreground" htmlFor="home-connect-switch">
        {t("actions.connect")}
      </Label>
      <div className="flex items-center gap-2">
        {connected ? (
          <Button
            aria-label={t("actions.restart")}
            className="gap-2"
            disabled={busy}
            onClick={onRestart}
            size="sm"
            type="button"
            variant="ghost"
          >
            <RotateCw className="size-4" aria-hidden="true" />
            {t("actions.restart")}
          </Button>
        ) : null}
        {busy ? <LoaderCircle aria-hidden="true" className="size-4 animate-spin text-muted-foreground" /> : null}
        <Switch
          aria-busy={busy || undefined}
          checked={checked}
          disabled={busy}
          id="home-connect-switch"
          onCheckedChange={() => onPrimaryAction()}
        />
      </div>
    </div>
  );
}

function SysProxyRow({
  onProxyModeChange,
  pacAvailable,
  proxyPending,
  requestedProxyMode,
  t,
}: {
  onProxyModeChange: (mode: SysProxyMode) => void;
  pacAvailable: boolean;
  proxyPending: SysProxyMode | null;
  requestedProxyMode: SysProxyMode;
  t: Translation;
}) {
  return (
    <div className="flex items-center justify-between gap-3 py-2.5">
      <span className="text-sm font-medium text-foreground">{t("status.sysProxyMode")}</span>
      <div className="flex flex-col items-end gap-1">
        <div
          aria-busy={proxyPending !== null}
          aria-label={t("status.sysProxyMode")}
          className="flex h-7 items-center rounded-md bg-muted p-0.5"
          role="group"
        >
          {PROXY_MODE_OPTIONS.map((mode) => {
            const selected = requestedProxyMode === mode;
            const modeLabel = sysProxyLabel(mode, t);
            const pacUnavailable = mode === "pac" && !pacAvailable;

            return (
              <Button
                key={mode}
                aria-describedby={pacUnavailable ? "home-pac-unavailable" : undefined}
                aria-label={modeLabel}
                aria-pressed={selected}
                className={cn(
                  "h-6 rounded-sm px-2.5 text-sm leading-none shadow-none focus-visible:relative focus-visible:z-10",
                  selected
                    ? "bg-background text-foreground hover:bg-background hover:text-foreground"
                    : "text-subtlest hover:bg-background/60 hover:text-foreground",
                )}
                disabled={proxyPending !== null || pacUnavailable}
                onClick={() => onProxyModeChange(mode)}
                size="sm"
                title={pacUnavailable ? t("status.sysProxyPacUnavailable") : undefined}
                type="button"
                variant="ghost"
              >
                {modeLabel}
              </Button>
            );
          })}
        </div>
        {!pacAvailable ? (
          <p className="max-w-64 text-end text-xs text-subtlest" id="home-pac-unavailable">
            {t("status.sysProxyPacUnavailable")}
          </p>
        ) : null}
      </div>
    </div>
  );
}

function TunRow({
  onTunToggle,
  t,
  tunEnabled,
  tunPending,
  tunProviderSummary,
}: {
  onTunToggle: () => void;
  t: Translation;
  tunEnabled: boolean;
  tunPending: boolean;
  tunProviderSummary: string | null;
}) {
  return (
    <div className="flex items-center justify-between gap-3 py-2.5">
      <div className="min-w-0">
        <Label className="text-sm font-medium text-foreground" htmlFor="home-tun-switch">
          {t("status.tun")}
        </Label>
        {tunProviderSummary ? <p className="mt-0.5 truncate text-xs text-subtlest">{tunProviderSummary}</p> : null}
      </div>
      <Switch checked={tunEnabled} disabled={tunPending} id="home-tun-switch" onCheckedChange={onTunToggle} />
    </div>
  );
}

function sysProxyLabel(mode: SysProxyMode, t: ReturnType<typeof useI18n>["t"]) {
  switch (mode) {
    case "forcedChange":
      return t("status.sysProxyGlobal");
    case "pac":
      return t("status.sysProxySmart");
    case "forcedClear":
    default:
      return t("status.sysProxyOff");
  }
}

function runtimeActionLabel(action: RuntimeAction, t: ReturnType<typeof useI18n>["t"]) {
  switch (action) {
    case "connect":
      return t("actions.connect");
    case "disconnect":
      return t("actions.disconnect");
    case "restart":
      return t("actions.restart");
  }
}

function tunProviderLabel(tun: TunChanged, t: Translation) {
  const backend = tunBackendLabel(tun.backend, t);
  const providerState = tunProviderStateLabel(tun.providerState, t);
  if (tun.lastProviderError) {
    return `${backend}: ${providerState}: ${tun.lastProviderError}`;
  }
  if (!tun.nativeComponentReady) {
    return `${backend}: ${providerState}`;
  }

  return `${backend}: ${providerState}`;
}

function tunProviderPathMismatchDescription(status: TunStatus, t: Translation) {
  return t("status.tunProviderPathMismatch", {
    expected: status.expectedProviderPath ?? "—",
    resolved: status.resolvedProviderPath ?? "—",
  });
}

function tunBackendLabel(backend: TunChanged["backend"], t: Translation) {
  switch (backend) {
    case "macosPacketTunnel":
      return t("status.tunBackendMacos");
    case "windowsService":
      return t("status.tunBackendWindows");
    case "process":
      return t("status.tunBackendProcess");
    case "unsupported":
    default:
      return t("status.tunBackendUnsupported");
  }
}

function tunProviderStateLabel(state: TunChanged["providerState"], t: Translation) {
  switch (state) {
    case "running":
      return t("status.tunProviderRunning");
    case "starting":
      return t("status.tunProviderStarting");
    case "stopped":
      return t("status.tunProviderStopped");
    case "permissionRequired":
      return t("status.tunProviderPermissionRequired");
    case "missingComponent":
      return t("status.tunProviderMissingComponent");
    case "error":
      return t("status.tunProviderError");
    case "notApplicable":
    default:
      return t("status.tunProviderNotApplicable");
  }
}
