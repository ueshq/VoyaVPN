import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  LoaderCircle,
  Power,
  PowerOff,
  RotateCw,
  Server,
  ShieldCheck,
  ShieldOff,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

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
  const [pendingAction, setPendingAction] = useState<RuntimeAction | null>(null);
  // TUN toggling is tracked separately from connect/disconnect so the two
  // controls never block each other.
  const [tunPending, setTunPending] = useState(false);
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
  const busy = inProgress || pendingAction !== null;

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
  const nodeLabel = activeProfile?.profile.Remarks || coreState?.activeProfileId || t("home.noNode");
  const requestedProxyMode = sysProxy?.requestedMode ?? "forcedClear";
  const tunEnabled = tun?.enabled ?? false;

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
      <div className="mx-auto flex w-full max-w-2xl flex-1 flex-col items-center justify-center gap-6 px-6 py-8">
        <div className="flex flex-col items-center gap-4 text-center">
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

        <div className="flex flex-col items-center gap-3">
          <Button
            aria-label={primaryLabel}
            className={cn("h-14 w-60 gap-2 rounded-lg text-base font-semibold", !connected && "shadow-raised")}
            disabled={busy}
            onClick={() => void runRuntimeAction(connected ? "disconnect" : "connect")}
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

        <div className="w-full rounded-lg bg-surface-raised px-4 shadow-raised">
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

        <dl className="w-full">
          <StatTile
            actionLabel={t("home.changeNode")}
            icon={Server}
            label={t("home.node")}
            mono
            onClick={() => openModal("nodePicker")}
            title={nodeLabel}
            value={nodeLabel}
          />
        </dl>
      </div>
    </section>
  );
}

function StatTile({
  actionLabel,
  icon: Icon,
  label,
  mono = false,
  onClick,
  title,
  value,
}: {
  actionLabel?: string;
  icon: LucideIcon;
  label: string;
  mono?: boolean;
  onClick?: () => void;
  title?: string;
  value: string;
}) {
  return (
    <div
      className={cn(
        "relative flex min-w-0 flex-col gap-1 rounded-lg bg-surface-raised px-3 py-2.5 shadow-raised",
        onClick && "transition-colors hover:bg-surface-overlay",
      )}
    >
      <dt className="flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wide text-subtle">
        <Icon className="size-3.5" aria-hidden="true" />
        <span className="min-w-0 truncate">{label}</span>
      </dt>
      <dd
        className={cn(
          "min-w-0 truncate text-sm font-medium text-foreground",
          mono && "font-mono text-xs",
        )}
        title={title ?? value}
      >
        {value}
      </dd>
      {onClick ? (
        // A stretched, transparent button keeps the `<dl>`/`<dt>`/`<dd>` markup
        // valid while giving the tile a real, keyboard-focusable activation target.
        <button
          aria-label={actionLabel ?? label}
          className="absolute inset-0 rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background"
          onClick={onClick}
          type="button"
        />
      ) : null}
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
