import { AppWindow, Info, Pencil } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { cn } from "@voya/ui/lib/utils";

import { PageSurface } from "@/components/app-shell/page-section";
import type { Routing_Serialize } from "@/ipc/bindings";

import { PER_APP_MODE_LABEL_KEYS, readPerAppRule } from "@voya/features/routing/per-app-proxy-rule";
import { useProcessRulesSupported } from "@voya/features/routing/use-process-rules-supported";
import { useConnectionModeStatus } from "@voya/features/routing/use-connection-mode-status";

const VISIBLE_APPS = 4;

/**
 * The per-app proxy rule at a glance. It is one managed rule pinned ahead of
 * every other rule, so the page shows it here rather than as a list row and
 * edits it through its own dialog.
 */
export function PerAppSummaryCard({
  locked = false,
  onEdit,
  routing,
}: {
  /** Global mode skips every rule, this one included. */
  locked?: boolean;
  onEdit: () => void;
  routing: Routing_Serialize | null;
}) {
  const { t } = useI18n();
  const { mode, processes } = readPerAppRule(routing);
  const statusQuery = useConnectionModeStatus();
  const processRulesSupported = useProcessRulesSupported();
  if (!processRulesSupported) return null;
  const on = mode !== "off";
  const shown = processes.slice(0, VISIBLE_APPS);
  const hidden = processes.length - shown.length;

  return (
    <PageSurface className="flex flex-wrap items-center gap-x-4 gap-y-3 px-4 py-3">
      <AppWindow aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
      <div className={cn("grid min-w-0 flex-1 gap-1.5", locked && "opacity-55")}>
        <div className="flex flex-wrap items-baseline gap-x-2">
          <h2 className="text-sm font-semibold">{t("panes.routing.perAppTitle")}</h2>
          <span className="text-sm text-muted-foreground">
            {t(PER_APP_MODE_LABEL_KEYS[mode])}
          </span>
        </div>
        {on ? (
          <div className="flex flex-wrap items-center gap-1.5">
            {shown.map((process) => (
              <Badge className="max-w-48" key={process.toLowerCase()} variant="secondary">
                <span className="truncate">{process}</span>
              </Badge>
            ))}
            {hidden > 0 ? (
              <span className="text-xs text-muted-foreground">
                {t("panes.routing.matchMore", { count: hidden })}
              </span>
            ) : null}
          </div>
        ) : null}
        <p className="text-xs text-muted-foreground">
          {on ? t("panes.routing.perAppPinnedHint") : t("panes.routing.perAppDescription")}
        </p>
        {on && statusQuery.data?.processRulesEffective === false ? (
          <p className="flex items-center gap-1.5 text-xs text-warning">
            <Info aria-hidden="true" className="size-3.5 shrink-0" />
            {t("panes.routing.perAppTunOnlyHint")}
          </p>
        ) : null}
      </div>
      {/* A disabled button shows no tooltip of its own. */}
      <span title={locked ? t("panes.routing.rulesLocked") : undefined}>
        <Button disabled={!routing || locked} onClick={onEdit} size="sm" type="button" variant="outline">
          <Pencil aria-hidden="true" className="size-4" />
          {t("actions.edit")}
        </Button>
      </span>
    </PageSurface>
  );
}
