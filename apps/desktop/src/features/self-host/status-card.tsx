import { ShieldAlert } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { cn } from "@voya/ui/lib/utils";
import { formatBytes } from "@voya/utils/formatting";

import { PageSurface } from "@/components/app-shell/page-section";
import type { SelfHostState, SelfHostStats } from "@/ipc/bindings";

import { PROBLEM_KEYS, STATUS_KEYS, statusTone } from "./self-host-labels";

const DOT_CLASS = {
  busy: "bg-warning",
  error: "bg-danger",
  off: "bg-muted-foreground",
  on: "bg-success",
} as const;

export function StatusCard({
  state,
  stats,
}: {
  state: SelfHostState;
  stats: SelfHostStats | null;
}) {
  const { t } = useI18n();
  const { config, runtime } = state;
  const tone = statusTone(runtime.status);

  return (
    <PageSurface className="grid gap-2 px-4 py-3" data-testid="self-host-status">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <span
          aria-hidden="true"
          className={cn("size-2.5 shrink-0 rounded-full", DOT_CLASS[tone])}
          data-state={tone}
        />
        <h2 className="text-section font-semibold">{t(STATUS_KEYS[runtime.status])}</h2>
        {stats ? (
          <dl className="ms-auto flex flex-wrap gap-x-5 gap-y-1 text-sm">
            <Stat label={t("panes.selfHost.stats.connections")} value={String(stats.activeConnections)} />
            <Stat label={t("panes.selfHost.stats.upload")} value={formatBytes(stats.uploadTotalBytes)} />
            <Stat label={t("panes.selfHost.stats.download")} value={formatBytes(stats.downloadTotalBytes)} />
          </dl>
        ) : null}
      </div>
      {runtime.problem ? (
        <p className="text-sm text-danger" role="status">
          {t(PROBLEM_KEYS[runtime.problem], { port: runtime.port ?? 0 })}
          {runtime.detail ? (
            <span className="ms-1 font-mono text-xs text-muted-foreground">{runtime.detail}</span>
          ) : null}
        </p>
      ) : null}
      {config.enabled ? null : (
        <p className="text-sm text-muted-foreground">{t("panes.selfHost.enableHint")}</p>
      )}
      <p className="flex items-start gap-1.5 text-xs text-muted-foreground">
        <ShieldAlert aria-hidden="true" className="mt-0.5 size-3.5 shrink-0 text-warning" />
        {t("panes.selfHost.exitWarning")}
      </p>
    </PageSurface>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline gap-1.5">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="font-medium tabular-nums">{value}</dd>
    </div>
  );
}
