import { useQuery } from "@tanstack/react-query";
import { AppWindow, Info, Pencil } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";

import { PageSurface } from "@/components/app-shell/page-section";
import type { Routing_Serialize } from "@/ipc/bindings";
import { connectionModeStatus } from "@/ipc/commands";
import { queryKeys } from "@/ipc/query-keys";

import { PER_APP_MODE_LABEL_KEYS, readPerAppRule } from "./per-app-proxy-rule";
import { useProcessRulesSupported } from "./use-process-rules-supported";

const VISIBLE_APPS = 4;

/**
 * The per-app proxy rule at a glance. It is one managed rule pinned ahead of
 * every other rule, so the page shows it here rather than as a list row and
 * edits it through its own dialog.
 */
export function PerAppSummaryCard({
  onEdit,
  routing,
}: {
  onEdit: () => void;
  routing: Routing_Serialize | null;
}) {
  const { t } = useI18n();
  const { mode, processes } = readPerAppRule(routing);
  const statusQuery = useQuery({
    queryFn: connectionModeStatus,
    queryKey: queryKeys.connectionMode,
  });
  const processRulesSupported = useProcessRulesSupported();
  if (!processRulesSupported) return null;
  const on = mode !== "off";
  const shown = processes.slice(0, VISIBLE_APPS);
  const hidden = processes.length - shown.length;

  return (
    <PageSurface className="flex flex-wrap items-center gap-x-4 gap-y-3 px-4 py-3">
      <AppWindow aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
      <div className="grid min-w-0 flex-1 gap-1.5">
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
      <Button disabled={!routing} onClick={onEdit} size="sm" type="button" variant="outline">
        <Pencil aria-hidden="true" className="size-4" />
        {t("actions.edit")}
      </Button>
    </PageSurface>
  );
}
