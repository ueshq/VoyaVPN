import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";
import { Switch } from "@voya/ui/components/switch";
import { cn } from "@voya/ui/lib/utils";
import { formatBytes } from "@voya/utils/formatting";

import { PageSurface } from "@/components/app-shell/page-section";
import type { SelfHostState } from "@/ipc/bindings";

import { NetworkStatus } from "./network-status";
import { PROBLEM_KEYS, STATUS_HINT_KEYS, STATUS_KEYS, statusTone } from "./self-host-labels";
import type { SelfHostController } from "./use-self-host";

const ENABLE_SWITCH_ID = "self-host-enabled";

const DOT_CLASS = {
  busy: "bg-warning",
  error: "bg-danger",
  off: "bg-muted-foreground",
  on: "bg-success",
} as const;

/**
 * The page's one question, answered first: is this device hosting a node, and
 * can other devices reach it. The switch and the state it produces sit
 * together; live traffic appears only while the node runs, and the network
 * check closes the card.
 */
export function HostingCard({
  controller,
  onOpenSettings,
  state,
}: {
  controller: SelfHostController;
  onOpenSettings: () => void;
  state: SelfHostState;
}) {
  const { t } = useI18n();
  const { runtime } = state;
  const { stats } = controller;
  const tone = statusTone(runtime.status);
  const hintKey = runtime.status === "running" || runtime.status === "stopped"
    ? STATUS_HINT_KEYS[runtime.status]
    : null;
  // Both problems are fixed in the settings dialog, so the way there is offered.
  const fixedInSettings = runtime.problem === "noProtocol" || runtime.problem === "portInUse";

  return (
    <PageSurface className="grid gap-4 p-5" data-testid="self-host-status">
      <div className="flex items-start gap-4">
        <div className="grid min-w-0 flex-1 gap-1.5">
          <label className="text-section font-semibold" htmlFor={ENABLE_SWITCH_ID}>
            {t("panes.selfHost.enable")}
          </label>
          <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm" role="status">
            <span
              aria-hidden="true"
              className={cn("size-2.5 shrink-0 rounded-full", DOT_CLASS[tone])}
              data-state={tone}
            />
            <span className="font-medium">{t(STATUS_KEYS[runtime.status])}</span>
            {hintKey && !runtime.problem ? (
              <span className="text-muted-foreground">{t(hintKey)}</span>
            ) : null}
          </p>
        </div>
        <Switch
          checked={state.config.enabled}
          className="mt-0.5"
          disabled={controller.pending === "enable"}
          id={ENABLE_SWITCH_ID}
          onCheckedChange={(enabled) => void controller.setEnabled(enabled)}
        />
      </div>
      {runtime.problem ? (
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <p className="min-w-0 text-sm text-danger" role="alert">
            {t(PROBLEM_KEYS[runtime.problem], { port: runtime.port ?? 0 })}
            {runtime.detail ? (
              <span className="ms-1 font-mono text-xs text-muted-foreground">{runtime.detail}</span>
            ) : null}
          </p>
          {fixedInSettings ? (
            <Button onClick={onOpenSettings} size="sm" type="button" variant="outline">
              {t("panes.selfHost.openSettings")}
            </Button>
          ) : null}
        </div>
      ) : null}
      {stats ? (
        <dl className="grid grid-cols-3 gap-4 border-t border-border-subtle pt-4">
          <Stat label={t("panes.selfHost.stats.connections")} value={String(stats.activeConnections)} />
          <Stat label={t("panes.selfHost.stats.upload")} value={formatBytes(stats.uploadTotalBytes)} />
          <Stat label={t("panes.selfHost.stats.download")} value={formatBytes(stats.downloadTotalBytes)} />
        </dl>
      ) : null}
      <NetworkStatus controller={controller} state={state} />
    </PageSurface>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid min-w-0 gap-0.5">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="truncate text-section font-semibold tabular-nums">{value}</dd>
    </div>
  );
}
