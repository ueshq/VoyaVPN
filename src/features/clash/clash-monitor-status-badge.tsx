import { AlertTriangle, CircleDot, LoaderCircle, PauseCircle } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { useI18n } from "@/i18n/use-i18n";
import type { RuntimeClashMonitorStatus } from "@/ipc/runtime-event-store";
import { cn } from "@/lib/utils";

type MonitorTone = "failed" | "live" | "starting" | "stale";

type MonitorStatusDisplay = {
  detail: string | null;
  label: string;
  tone: MonitorTone;
};

const toneClassName: Record<MonitorTone, string> = {
  failed: "border-destructive/35 bg-destructive/10 text-destructive [&>svg]:text-destructive",
  live: "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300 [&>svg]:text-emerald-600 dark:[&>svg]:text-emerald-300",
  starting: "border-sky-500/30 bg-sky-500/10 text-sky-700 dark:text-sky-300 [&>svg]:text-sky-600 dark:[&>svg]:text-sky-300",
  stale: "border-amber-500/35 bg-amber-500/10 text-amber-800 dark:text-amber-300 [&>svg]:text-amber-600 dark:[&>svg]:text-amber-300",
};

export function ClashMonitorStatusBadge({
  className,
  status,
}: {
  className?: string;
  status: RuntimeClashMonitorStatus;
}) {
  const { t } = useI18n();
  const display = monitorStatusDisplay(status, t);
  const title = [display.label, display.detail].filter(Boolean).join(": ");
  const iconClassName = cn("size-3 shrink-0", status.state === "starting" && "animate-spin");

  return (
    <span className={cn("min-w-0 max-w-[18rem]", className)}>
      <Badge
        aria-label={title}
        className={cn(
          "w-full min-w-0 justify-start gap-1.5 px-2 py-1 font-normal",
          toneClassName[display.tone],
        )}
        role="status"
        title={title}
        variant="outline"
      >
        {status.state === "failed" ? <AlertTriangle className={iconClassName} aria-hidden="true" /> : null}
        {status.state === "starting" ? <LoaderCircle className={iconClassName} aria-hidden="true" /> : null}
        {status.state !== "failed" && status.state !== "starting" && status.stale ? (
          <PauseCircle className={iconClassName} aria-hidden="true" />
        ) : null}
        {status.state !== "failed" && status.state !== "starting" && !status.stale ? (
          <CircleDot className={iconClassName} aria-hidden="true" />
        ) : null}
        <span className="shrink-0">{display.label}</span>
        {display.detail ? <span className="min-w-0 truncate opacity-80">{display.detail}</span> : null}
      </Badge>
    </span>
  );
}

function monitorStatusDisplay(
  status: RuntimeClashMonitorStatus,
  t: (key: string, options?: Record<string, unknown>) => string,
): MonitorStatusDisplay {
  if (status.state === "failed") {
    return {
      detail: status.message,
      label: t("clash.monitorFailed"),
      tone: "failed",
    };
  }

  if (status.state === "starting") {
    return {
      detail: status.stale ? t("clash.monitorStale") : null,
      label: t("clash.monitorStarting"),
      tone: status.stale ? "stale" : "starting",
    };
  }

  if (!status.stale && status.running) {
    return {
      detail: status.message,
      label: t("clash.monitorLive"),
      tone: "live",
    };
  }

  return {
    detail: status.state === "stopped" ? t("clash.monitorStopped") : status.message,
    label: t("clash.monitorStale"),
    tone: "stale",
  };
}
