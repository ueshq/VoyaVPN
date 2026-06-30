import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { LoaderCircle, Power, PowerOff, RotateCw, ShieldCheck, ShieldOff } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { useI18n } from "@/i18n/use-i18n";
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
import type { SysProxyMode } from "@/ipc/bindings";
import { cn, getErrorMessage } from "@/lib/utils";
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

/**
 * Connection home Hero — the default view and signature surface. Single-accent
 * discipline: the idle connect CTA is brand blue (`--primary`); affirmative green
 * (`--connected` / `--connected-glow`) is reserved for the achieved protected
 * state (status disc + headline). It only reuses the existing runtime actions and
 * {@link useRuntimeEventStore}; no new IPC is introduced. Decorative motion (the
 * status-light spinner) inherits the global `prefers-reduced-motion` guard in
 * globals.css.
 */
export function HomeScreen() {
  const { t } = useI18n();
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
      .catch(() => undefined);
    void tunStatus()
      .then((status) => {
        if (!cancelled) {
          setTun(statusToTunChanged(status));
        }
      })
      .catch(() => undefined);

    return () => {
      cancelled = true;
    };
  }, [setSysProxy, setTun]);

  const state = coreState?.state ?? "disconnected";
  const connected = state === "connected";
  const inProgress = state === "connecting" || state === "disconnecting";
  const busy = inProgress || pendingAction !== null || switchingId !== null;

  const headline = connected
    ? t("home.protected")
    : state === "connecting"
      ? t("status.connecting")
      : state === "disconnecting"
        ? t("status.disconnecting")
        : t("home.unprotected");
  const hint = connected
    ? t("home.protectedHint")
    : state === "disconnected"
      ? t("home.unprotectedHint")
      : "";

  const activeProfile = profilesQuery.data?.find((item) => item.isActive) ?? null;
  const activeProfileId = activeProfile?.profile.IndexId ?? null;
  // The green "live" dot follows the node that is actually running, which differs
  // from the persisted-active node only while disconnected.
  const runningId = connected ? (coreState?.activeProfileId ?? null) : null;
  const requestedProxyMode = sysProxy?.requestedMode ?? "forcedClear";
  const tunEnabled = tun?.enabled ?? false;

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
    } catch {
      return;
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
    try {
      const status = await setSystemProxyMode(SYS_PROXY_TYPE[mode]);
      setSysProxy(statusToSysProxyChanged(status));
    } catch {
      return;
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
      setTunPending(false);
    }
  }

  const StatusIcon = connected ? ShieldCheck : inProgress ? LoaderCircle : ShieldOff;
  const primaryLabel = connected ? t("actions.disconnect") : t("actions.connect");
  const PrimaryIcon = busy ? LoaderCircle : connected ? PowerOff : Power;

  return (
    <section
      aria-label={t("home.aria")}
      className="flex h-full min-h-0 flex-col overflow-y-auto"
      data-testid="home-screen"
      role="region"
    >
      <div className="mx-auto flex w-full min-h-0 max-w-2xl flex-1 flex-col items-center gap-6 px-6 py-8">
        <div className="flex shrink-0 flex-col items-center gap-4 text-center">
          <span
            aria-hidden="true"
            className={cn(
              "flex size-20 items-center justify-center rounded-full border transition-colors",
              connected
                ? "border-connected/40 bg-connected/10 text-connected shadow-[var(--connected-glow)]"
                : "border-border bg-surface-sunken text-muted-foreground",
            )}
          >
            <StatusIcon className={cn("size-9", inProgress && "animate-spin")} />
          </span>
          <div className="space-y-1">
            <p
              className={cn(
                "font-display text-2xl font-semibold tracking-tight",
                connected ? "text-connected" : "text-foreground",
              )}
            >
              {headline}
            </p>
            {hint ? <p className="text-sm text-muted-foreground">{hint}</p> : null}
          </div>
        </div>

        <div className="flex shrink-0 flex-col items-center gap-3">
          <Button
            aria-label={primaryLabel}
            className={cn("h-14 w-60 gap-2 rounded-lg text-base font-semibold", !connected && "shadow-raised")}
            disabled={busy}
            onClick={handlePrimaryAction}
            size="lg"
            type="button"
            variant={connected ? "outline" : "default"}
          >
            <PrimaryIcon className={cn("size-5", busy && "animate-spin")} aria-hidden="true" />
            {primaryLabel}
          </Button>
          {connected ? (
            <Button
              aria-label={t("actions.restart")}
              className="gap-2"
              disabled={busy}
              onClick={() => void runRuntimeAction("restart")}
              size="sm"
              type="button"
              variant="ghost"
            >
              <RotateCw className="size-4" aria-hidden="true" />
              {t("actions.restart")}
            </Button>
          ) : null}
        </div>

        <div className="w-full shrink-0 rounded-lg bg-surface-raised px-4 shadow-raised">
          <div className="flex items-center justify-between gap-3 py-2.5">
            <span className="text-sm font-medium text-foreground">{t("status.sysProxyMode")}</span>
            <div
              aria-label={t("status.sysProxyMode")}
              className="flex h-7 items-center rounded-md bg-muted p-0.5"
              role="group"
            >
              {PROXY_MODE_OPTIONS.map((mode) => {
                const selected = requestedProxyMode === mode;
                const modeLabel = sysProxyLabel(mode, t);

                return (
                  <Button
                    key={mode}
                    aria-label={modeLabel}
                    aria-pressed={selected}
                    className={cn(
                      "h-6 rounded-sm px-2.5 text-sm leading-none shadow-none focus-visible:relative focus-visible:z-10",
                      selected
                        ? "bg-background text-foreground hover:bg-background hover:text-foreground"
                        : "text-subtlest hover:bg-background/60 hover:text-foreground",
                    )}
                    onClick={() => void runProxyMode(mode)}
                    size="sm"
                    type="button"
                    variant="ghost"
                  >
                    {modeLabel}
                  </Button>
                );
              })}
            </div>
          </div>
          <Separator />
          <div className="flex items-center justify-between gap-3 py-2.5">
            <Label className="text-sm font-medium text-foreground" htmlFor="home-tun-switch">
              {t("status.tun")}
            </Label>
            <Switch
              checked={tunEnabled}
              disabled={tunPending}
              id="home-tun-switch"
              onCheckedChange={() => void runTunToggle()}
            />
          </div>
        </div>

        <NodeList
          isPending={profilesQuery.isPending}
          onActivate={(indexId) => void switchActiveAndApply(indexId)}
          onSelect={(indexId) => setSelectedId(indexId)}
          profiles={profilesQuery.data ?? []}
          runningId={runningId}
          selectedId={selectedId}
          switchingId={switchingId}
        />
      </div>
    </section>
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
