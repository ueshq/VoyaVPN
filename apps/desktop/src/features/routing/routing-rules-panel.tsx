import { ArrowDown, ArrowUp, Pencil, Plus, Route, Trash2 } from "lucide-react";

import {
  dataTableHeader,
  dataTableRowEven,
  dataTableRowHover,
  dataTableRowOdd,
  dataTableRowSelected,
} from "@/components/app-shell/data-table-surface";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { EmptyState } from "@voya/ui/components/empty-state";
import { ScrollArea } from "@voya/ui/components/scroll-area";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@voya/ui/components/table";
import { cn } from "@voya/ui/lib/utils";
import { MOVE_ACTIONS } from "@/features/profiles/profile-constants";
import { PageHeader, PageHeaderActions } from "@/components/app-shell/page-section";
import type { RoutingRuleScope } from "@/ipc/bindings";
import { useI18n } from "@voya/i18n/use-i18n";

import { RULE_TYPES } from "./routing-constants";
import type { RoutingScreenController } from "./use-routing-screen";

export function RoutingRulesPanel({ controller }: { controller: RoutingScreenController }) {
  const { t } = useI18n();
  const {
    moveSelectedRule,
    requestDeleteRule,
    selectedRouting,
    selectedRule,
    setRuleDialog,
    setSelectedRuleId,
  } = controller;

  return (
    <div className="flex min-h-0 min-w-0 flex-col">
      <PageHeader className="min-h-14">
        <div className="min-w-0">
          <h3 className="truncate text-sm font-semibold">
            {selectedRouting?.remarks ?? t("panes.routing.noProfile")}
          </h3>
          <p className="truncate text-xs text-muted-foreground">
            {selectedRouting
              ? t("panes.routing.rulesSummary", {
                  count: selectedRouting.rules.length,
                  strategy: selectedRouting.singboxDomainStrategy || t("panes.routing.defaultValue"),
                })
              : ""}
          </p>
        </div>
        <PageHeaderActions>
          <Button disabled={!selectedRouting} onClick={() => setRuleDialog({ mode: "create" })} size="sm" type="button">
            <Plus className="size-4" aria-hidden="true" />
            {t("panes.routing.rule")}
          </Button>
          <Button
            disabled={!selectedRule}
            onClick={() => selectedRule && setRuleDialog({ mode: "edit", rule: selectedRule })}
            size="sm"
            type="button"
            variant="outline"
          >
            <Pencil className="size-4" aria-hidden="true" />
            {t("actions.edit")}
          </Button>
          <Button
            aria-label={t("panes.routing.moveRuleUp")}
            disabled={!selectedRouting || !selectedRule}
            onClick={() => moveSelectedRule(MOVE_ACTIONS.Up)}
            size="icon"
            type="button"
            variant="outline"
          >
            <ArrowUp className="size-4" aria-hidden="true" />
          </Button>
          <Button
            aria-label={t("panes.routing.moveRuleDown")}
            disabled={!selectedRouting || !selectedRule}
            onClick={() => moveSelectedRule(MOVE_ACTIONS.Down)}
            size="icon"
            type="button"
            variant="outline"
          >
            <ArrowDown className="size-4" aria-hidden="true" />
          </Button>
          <Button
            disabled={!selectedRouting || !selectedRule}
            onClick={requestDeleteRule}
            size="sm"
            type="button"
            variant="outline"
          >
            <Trash2 className="size-4" aria-hidden="true" />
            {t("actions.delete")}
          </Button>
        </PageHeaderActions>
      </PageHeader>

      {/* Radix's intrinsic-width wrapper must not expand the grid to the table's
          minimum width. The table keeps its own horizontal scroll container. */}
      <ScrollArea
        className={cn(
          "min-h-0 min-w-0 flex-1 [&_[data-slot=scroll-area-viewport]>div]:block! [&_[data-slot=scroll-area-viewport]>div]:h-full",
          selectedRouting?.rules.length ? "bg-surface-sunken" : "",
        )}
      >
        {(selectedRouting?.rules ?? []).length > 0 ? (
          <Table className="min-w-[58rem]">
            <TableHeader className={cn("sticky top-0 z-10", dataTableHeader)}>
              <TableRow className="hover:bg-transparent">
                <TableHead className="w-12 px-3 text-muted-foreground" scope="col">
                  #
                </TableHead>
                <TableHead className="px-3 text-muted-foreground" scope="col">
                  {t("panes.routing.remarks")}
                </TableHead>
                <TableHead className="px-3 text-muted-foreground" scope="col">
                  {t("panes.routing.outbound")}
                </TableHead>
                <TableHead className="px-3 text-muted-foreground" scope="col">
                  {t("panes.routing.type")}
                </TableHead>
                <TableHead className="px-3 text-muted-foreground" scope="col">
                  {t("panes.routing.domain")}
                </TableHead>
                <TableHead className="px-3 text-muted-foreground" scope="col">
                  IP
                </TableHead>
                <TableHead className="px-3 text-muted-foreground" scope="col">
                  {t("panes.routing.port")}
                </TableHead>
                <TableHead className="px-3 text-muted-foreground" scope="col">
                  {t("panes.routing.network")}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {(selectedRouting?.rules ?? []).map((rule, index) => (
                <TableRow
                  className={cn(
                    "cursor-default",
                    selectedRule?.id === rule.id
                      ? dataTableRowSelected
                      : cn(index % 2 === 0 ? dataTableRowEven : dataTableRowOdd, dataTableRowHover),
                    !rule.enabled ? "opacity-55" : "",
                  )}
                  key={rule.id}
                  onClick={() => setSelectedRuleId(rule.id)}
                >
                  <TableCell className="px-3 py-2 tabular-nums text-muted-foreground">{index + 1}</TableCell>
                  <TableCell className="max-w-52 truncate px-3 py-2 font-medium">{rule.remarks ?? ""}</TableCell>
                  <TableCell className="px-3 py-2">{rule.outbound ?? ""}</TableCell>
                  <TableCell className="px-3 py-2">
                    <RuleTypeBadge scope={rule.scope} />
                  </TableCell>
                  <TableCell className="max-w-72 truncate px-3 py-2">{formatList(rule.domain)}</TableCell>
                  <TableCell className="max-w-56 truncate px-3 py-2">{formatList(rule.ip)}</TableCell>
                  <TableCell className="px-3 py-2">{rule.port ?? ""}</TableCell>
                  <TableCell className="px-3 py-2">{rule.network ?? ""}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        ) : (
          <EmptyState className="h-full content-center" icon={Route} title={t("panes.routing.emptyRules")} />
        )}
      </ScrollArea>
    </div>
  );
}

function RuleTypeBadge({ scope }: { scope: RoutingRuleScope | null | undefined }) {
  const { t } = useI18n();
  return (
    <Badge className="bg-background" variant="outline">
      {formatRuleType(scope, t)}
    </Badge>
  );
}

function formatRuleType(scope: RoutingRuleScope | null | undefined, t: ReturnType<typeof useI18n>["t"]) {
  switch (scope) {
    case RULE_TYPES.Routing:
      return t("panes.routing.scopeRouting");
    case RULE_TYPES.Dns:
      return "DNS";
    case RULE_TYPES.All:
    default:
      return t("panes.routing.scopeAll");
  }
}

function formatList(values: string[] | null | undefined) {
  return values?.join(", ") ?? "";
}
