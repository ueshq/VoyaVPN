import { AppWindow, CheckCircle2, Pencil, Play, Plus, Route, Trash2 } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@voya/ui/components/alert-dialog";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { buttonVariants } from "@voya/ui/components/button-variants";
import { EmptyState } from "@voya/ui/components/empty-state";
import { ScrollArea } from "@voya/ui/components/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@voya/ui/components/select";
import { cn } from "@voya/ui/lib/utils";

import { dataTableRowSelected } from "@/components/app-shell/data-table-surface";
import { PageContent, PageHeader, PageSection, PageSurface, PageTitle } from "@/components/app-shell/page-section";
import { InlinePageError } from "@/components/app-shell/inline-page-error";
import { Toolbar } from "@/components/app-shell/toolbar";
import { useShellStore } from "@/stores/shell-store";

import { PerAppProxyDialog } from "./per-app-proxy-dialog";
import { RoutingProfileDialog } from "./routing-profile-dialog";
import { RoutingQuickSettings } from "./routing-quick-settings";
import { RoutingRuleDialog } from "./routing-rule-dialog";
import { RoutingRulesPanel } from "./routing-rules-panel";
import { useRoutingScreen, type RoutingScreenController } from "./use-routing-screen";

export function RoutingScreen() {
  const { t } = useI18n();
  const controller = useRoutingScreen();

  return (
    <PageSection aria-label={t("tabs.rules")}>
      <PageTitle
        title={t("tabs.rules")}
        actions={<RoutingToolbar controller={controller} />}
      />
      <PageContent>
        {controller.operationError ? (
          <InlinePageError>{controller.operationError}</InlinePageError>
        ) : null}
        <RoutingQuickSettings controller={controller} />
        <PageSurface className="@container/routing flex min-h-0 flex-1 flex-col overflow-hidden">
          <div className="p-4 @min-[896px]/routing:hidden">
            <Select
              value={controller.selectedRouting?.id ?? ""}
              onValueChange={controller.selectRouting}
            >
              <SelectTrigger
                className="w-full"
                aria-label={t("panes.routing.chooseProfile")}
              >
                <SelectValue placeholder={t("panes.routing.chooseProfile")} />
              </SelectTrigger>
              <SelectContent>
                {controller.routings.map((routing) => (
                  <SelectItem key={routing.id} value={routing.id}>
                    {routing.remarks || t("panes.routing.untitled")}
                    {routing.isActive ? ` · ${t("panes.routing.active")}` : ""}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <div className="grid min-h-0 flex-1 grid-cols-1 @min-[896px]/routing:grid-cols-[18rem_minmax(0,1fr)]">
            <div className="hidden min-h-0 min-w-0 @min-[896px]/routing:flex">
              <RoutingProfileList controller={controller} />
            </div>
            <RoutingRulesPanel controller={controller} />
          </div>
        </PageSurface>
      </PageContent>

      <RoutingDialogs controller={controller} />
    </PageSection>
  );
}

function RoutingToolbar({
  controller,
}: {
  controller: RoutingScreenController;
}) {
  const { t } = useI18n();
  const perAppOpen = useShellStore((state) => state.routingPerAppRequested);
  const setPerAppOpen = (open: boolean) =>
    useShellStore.setState({ routingPerAppRequested: open });
  const {
    activateSelectedRouting,
    requestDeleteRouting,
    selectedRouting,
    setRoutingDialog,
  } = controller;

  return (
    <Toolbar className="justify-end">
      <Button
        onClick={() => setRoutingDialog({ mode: "create" })}
        size="sm"
        type="button"
      >
        <Plus className="size-4" aria-hidden="true" />
        {t("panes.routing.profile")}
      </Button>
      <Button
        disabled={!selectedRouting}
        onClick={() =>
          selectedRouting &&
          setRoutingDialog({ mode: "edit", routing: selectedRouting })
        }
        size="sm"
        type="button"
        variant="outline"
      >
        <Pencil className="size-4" aria-hidden="true" />
        {t("actions.edit")}
      </Button>
      <Button
        disabled={!selectedRouting || selectedRouting.isActive}
        onClick={activateSelectedRouting}
        size="sm"
        type="button"
        variant="outline"
      >
        <Play className="size-4" aria-hidden="true" />
        {t("actions.activate")}
      </Button>
      <Button
        disabled={!selectedRouting}
        onClick={requestDeleteRouting}
        size="sm"
        type="button"
        variant="outline"
      >
        <Trash2 className="size-4" aria-hidden="true" />
        {t("actions.delete")}
      </Button>
      <Button
        onClick={() => setPerAppOpen(true)}
        size="sm"
        type="button"
        variant="outline"
      >
        <AppWindow className="size-4" aria-hidden="true" />
        {t("panes.routing.perAppTitle")}
      </Button>

      {perAppOpen ? (
        <PerAppProxyDialog onOpenChange={setPerAppOpen} open={perAppOpen} />
      ) : null}
    </Toolbar>
  );
}

function RoutingProfileList({ controller }: { controller: RoutingScreenController }) {
  const { t } = useI18n();
  const { routings, selectRouting, selectedRouting } = controller;

  return (
    <aside className="flex min-h-0 min-w-0 flex-1 flex-col">
      <PageHeader className="min-h-14 text-xs font-medium uppercase text-muted-foreground">
        {t("panes.routing.profiles")}
      </PageHeader>
      <ScrollArea className="min-h-0 flex-1">
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

function RoutingDialogs({ controller }: { controller: RoutingScreenController }) {
  const { t } = useI18n();
  const {
    confirmDelete,
    handleSaveRouting,
    handleSaveRule,
    pendingDelete,
    routingDialog,
    ruleDialog,
    selectedRouting,
    setPendingDelete,
    setRoutingDialog,
    setRuleDialog,
  } = controller;
  const deletingRouting = pendingDelete === "routing";
  const resettingRules = pendingDelete === "reset";
  const routingName = selectedRouting?.remarks || t("panes.routing.untitled");

  return (
    <>
      <RoutingProfileDialog
        key={routingDialog?.mode === "edit" ? `routing-${routingDialog.routing.id}` : `routing-${routingDialog?.mode ?? "closed"}`}
        mode={routingDialog?.mode ?? "create"}
        onOpenChange={(open) => !open && setRoutingDialog(null)}
        onSubmit={handleSaveRouting}
        open={Boolean(routingDialog)}
        routing={routingDialog?.mode === "edit" ? routingDialog.routing : null}
      />
      <RoutingRuleDialog
        key={ruleDialog?.mode === "edit" ? `rule-${ruleDialog.rule.id}` : `rule-${ruleDialog?.mode ?? "closed"}`}
        mode={ruleDialog?.mode ?? "create"}
        onOpenChange={(open) => !open && setRuleDialog(null)}
        onSubmit={handleSaveRule}
        open={Boolean(ruleDialog)}
        rule={ruleDialog?.mode === "edit" ? ruleDialog.rule : null}
      />
      {/* Deleting a routing profile also deletes every rule it owns, and both
          Delete buttons are enabled as soon as anything exists, so a misclick
          would be unrecoverable. Same AlertDialog gate the profiles table uses. */}
      <AlertDialog open={pendingDelete !== null} onOpenChange={(open) => !open && setPendingDelete(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {resettingRules
                ? t("confirm.resetRoutingRulesTitle")
                : deletingRouting
                  ? t("confirm.deleteRoutingTitle")
                  : t("confirm.deleteRoutingRuleTitle")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {resettingRules
                ? t("confirm.resetRoutingRulesDescription", { name: routingName })
                : deletingRouting
                  ? t("confirm.deleteRoutingDescription", {
                      count: selectedRouting?.rules.length ?? 0,
                      name: routingName,
                    })
                  : t("confirm.deleteRoutingRuleDescription")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("confirm.cancel")}</AlertDialogCancel>
            <AlertDialogAction className={buttonVariants({ variant: "destructive" })} onClick={confirmDelete}>
              {resettingRules
                ? t("confirm.resetRoutingRulesConfirm")
                : deletingRouting
                  ? t("confirm.deleteRoutingConfirm")
                  : t("confirm.deleteRoutingRuleConfirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
