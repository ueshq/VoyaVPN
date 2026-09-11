import { CheckCircle2, Route } from "lucide-react";

import { dataTableRowSelected } from "@/components/app-shell/data-table-surface";
import { Badge } from "@voya/ui/components/badge";
import { EmptyState } from "@voya/ui/components/empty-state";
import { ScrollArea } from "@voya/ui/components/scroll-area";
import { cn } from "@voya/ui/lib/utils";
import { useI18n } from "@voya/i18n/use-i18n";

import type { RoutingScreenController } from "./use-routing-screen";

export function RoutingProfileList({ controller }: { controller: RoutingScreenController }) {
  const { t } = useI18n();
  const { routings, selectRouting, selectedRouting } = controller;

  return (
    <aside className="min-h-0 border-b lg:border-b-0 lg:border-e">
      <div className="h-10 border-b px-4 py-2 text-xs font-medium uppercase text-muted-foreground">
        {t("panes.routing.profiles")}
      </div>
      <ScrollArea className="h-[18rem] lg:h-full">
        {routings.length > 0 ? (
          <div className="p-2">
            {routings.map((routing) => (
              <button
                className={cn(
                  "mb-1 flex min-h-14 w-full items-center gap-3 rounded-lg px-3 py-2 text-start outline-none transition-colors focus-visible:ring-[3px] focus-visible:ring-ring/50",
                  selectedRouting?.id === routing.id ? dataTableRowSelected : "hover:bg-surface-hovered",
                )}
                key={routing.id}
                onClick={() => selectRouting(routing.id)}
                type="button"
              >
                <span className="grid size-6 shrink-0 place-items-center rounded-md border bg-surface-raised">
                  {routing.isActive ? <CheckCircle2 className="size-4 text-success" aria-hidden="true" /> : null}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="line-clamp-1 text-sm font-medium">
                    {routing.remarks || t("panes.routing.untitled")}
                  </span>
                  <span className="block truncate text-xs text-muted-foreground">
                    {t("panes.routing.rulesCount", { count: routing.rules.length })}{" "}
                    {routing.singboxDomainStrategy || t("panes.routing.defaultValue")}
                  </span>
                </span>
                {routing.isActive ? (
                  <Badge className="shrink-0 border-connected/30 bg-connected/10 text-success" variant="outline">
                    {t("panes.routing.active")}
                  </Badge>
                ) : null}
              </button>
            ))}
          </div>
        ) : (
          <EmptyState className="py-10" icon={Route} title={t("panes.routing.emptyProfiles")} />
        )}
      </ScrollArea>
    </aside>
  );
}
