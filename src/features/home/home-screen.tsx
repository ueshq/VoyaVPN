import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  ArrowDown,
  ArrowUp,
  Clock,
  Cpu,
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
import { useI18n } from "@/i18n/use-i18n";
import {
  connectActiveProfile,
  disconnectCore,
  listProfiles,
  restartCore,
  useRuntimeEventStore,
} from "@/ipc";
import { formatCoreType } from "@/lib/core-types";
import { formatBytesPerSecond } from "@/lib/formatting";
import { cn } from "@/lib/utils";
import { useModalStore } from "@/stores/modal-store";

import { missingCorePayload, runWithElevation, statusToCoreState } from "./runtime-action";

type RuntimeAction = "connect" | "disconnect" | "restart";

/**
 * Connection home Hero — the default view. It is the signature surface: a calm
 * slate canvas while disconnected, lit with the brand `--signal` / `--glow-signal`
 * once protected. It only reuses the existing runtime actions and
 * {@link useRuntimeEventStore}; no new IPC is introduced. Decorative motion (the
 * status-light spinner) inherits the global `prefers-reduced-motion` guard in
 * globals.css.
 */
export function HomeScreen() {
  const { t } = useI18n();
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const setCoreState = useRuntimeEventStore((state) => state.setCoreState);
  const statistics = useRuntimeEventStore((state) => state.statistics);
  const openModal = useModalStore((state) => state.openModal);
  const [pendingAction, setPendingAction] = useState<RuntimeAction | null>(null);
  // Shares the ProfilesScreen query cache (same key) so resolving the active
  // node's name here costs no extra fetch and stays in sync after a switch.
  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, null),
    queryKey: ["profiles", { filter: "" }],
  });

  const state = coreState?.state ?? "disconnected";
  const connected = state === "connected";
  const inProgress = state === "connecting" || state === "disconnecting";
  const busy = inProgress || pendingAction !== null;

  const duration = useConnectedDuration(connected);

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
  const coreLabel = coreState?.runningCoreType ? formatCoreType(coreState.runningCoreType) : t("status.noCore");
  const uploadLabel = formatBytesPerSecond(statistics?.uploadBytesPerSecond ?? 0);
  const downloadLabel = formatBytesPerSecond(statistics?.downloadBytesPerSecond ?? 0);
  const durationLabel = connected ? formatDuration(duration) : "—";

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
      }
    } finally {
      setPendingAction(null);
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
      <div className="mx-auto flex w-full max-w-2xl flex-1 flex-col items-center justify-center gap-8 px-6 py-10">
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
                "font-display text-3xl font-semibold tracking-tight",
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
            className="h-14 w-60 gap-2 text-base font-semibold"
            disabled={busy}
            onClick={() => void runRuntimeAction(connected ? "disconnect" : "connect")}
            size="lg"
            type="button"
            variant={connected ? "outline" : "signal"}
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

        <dl className="grid w-full grid-cols-2 gap-3 sm:grid-cols-3">
          <StatTile
            actionLabel={t("home.changeNode")}
            icon={Server}
            label={t("home.node")}
            mono
            onClick={() => openModal("nodePicker")}
            title={nodeLabel}
            value={nodeLabel}
          />
          <StatTile icon={Cpu} label={t("home.core")} value={coreLabel} />
          <StatTile emphasis icon={Clock} label={t("home.duration")} value={durationLabel} />
          <StatTile emphasis icon={ArrowUp} label={t("home.upload")} value={uploadLabel} />
          <StatTile emphasis icon={ArrowDown} label={t("home.download")} value={downloadLabel} />
        </dl>
      </div>
    </section>
  );
}

function StatTile({
  actionLabel,
  emphasis = false,
  icon: Icon,
  label,
  mono = false,
  onClick,
  title,
  value,
}: {
  actionLabel?: string;
  emphasis?: boolean;
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
        "relative flex min-w-0 flex-col gap-1 rounded-xl bg-surface-raised px-3 py-2.5 shadow-raised",
        onClick && "transition-colors hover:bg-surface-overlay",
      )}
    >
      <dt className="flex items-center gap-1.5 text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
        <Icon className="size-3.5" aria-hidden="true" />
        <span className="min-w-0 truncate">{label}</span>
      </dt>
      <dd
        className={cn(
          "min-w-0 truncate text-sm font-medium text-foreground",
          emphasis && "font-display text-base tabular-nums",
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
          className="absolute inset-0 rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background"
          onClick={onClick}
          type="button"
        />
      ) : null}
    </div>
  );
}

function useConnectedDuration(connected: boolean) {
  const [elapsedMs, setElapsedMs] = useState(0);

  useEffect(() => {
    if (!connected) {
      return undefined;
    }

    const startedAt = Date.now();
    const interval = window.setInterval(() => setElapsedMs(Math.max(0, Date.now() - startedAt)), 1_000);

    return () => {
      window.clearInterval(interval);
      setElapsedMs(0);
    };
  }, [connected]);

  return connected ? elapsedMs : 0;
}

function formatDuration(ms: number) {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const pad = (value: number) => String(value).padStart(2, "0");

  return hours > 0 ? `${hours}:${pad(minutes)}:${pad(seconds)}` : `${minutes}:${pad(seconds)}`;
}
