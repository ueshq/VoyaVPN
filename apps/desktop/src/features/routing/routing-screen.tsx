import { useState } from "react";
import { Plus, RotateCcw, Route } from "lucide-react";

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
import { Button } from "@voya/ui/components/button";
import { buttonVariants } from "@voya/ui/components/button-variants";
import { EmptyState } from "@voya/ui/components/empty-state";
import { ScrollArea } from "@voya/ui/components/scroll-area";
import { cn } from "@voya/ui/lib/utils";

import { InlinePageError } from "@/components/app-shell/inline-page-error";
import {
  PageContent,
  PageHeader,
  PageHeaderActions,
  PageSection,
  PageSurface,
  PageTitle,
} from "@/components/app-shell/page-section";
import { useShellStore } from "@/stores/shell-store";

import { PerAppProxyDialog } from "./per-app-proxy-dialog";
import { PerAppSummaryCard } from "./per-app-summary-card";
import { RoutingRuleDialog } from "./routing-rule-dialog";
import { RoutingRuleList } from "./routing-rule-list";
import { ruleDisplayName } from "./sentinel-rules";
import { TrafficModeBanner } from "./traffic-mode-banner";
import { useProcessRulesSupported } from "./use-process-rules-supported";
import { useRoutingScreen, type RoutingScreenController } from "./use-routing-screen";

export function RoutingScreen() {
  const { t } = useI18n();
  const controller = useRoutingScreen();
  const perAppOpen = useShellStore((state) => state.routingPerAppRequested);
  const processRulesSupported = useProcessRulesSupported();
  const { activeRouting, ruleDialog } = controller;
  const error = controller.operationError ?? controller.loadError;

  return (
    <PageSection aria-label={t("tabs.rules")}>
      <PageTitle
        actions={
          <Button
            disabled={!activeRouting}
            onClick={controller.requestResetRules}
            size="sm"
            type="button"
            variant="outline"
          >
            <RotateCcw className="size-4" aria-hidden="true" />
            {t("panes.routing.resetRules")}
          </Button>
        }
        title={t("tabs.rules")}
      />
      <PageContent>
        {error ? <InlinePageError>{error}</InlinePageError> : null}
        <TrafficModeBanner />
        <PerAppSummaryCard
          onEdit={() => controller.setPerAppOpen(true)}
          routing={activeRouting}
        />
        <PageSurface className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <PageHeader className="min-h-14">
            <p className="min-w-0 flex-1 text-sm text-muted-foreground">
              {t("panes.routing.ruleOrderHint")}
            </p>
            <PageHeaderActions>
              <Button
                disabled={!activeRouting}
                onClick={controller.openCreateRule}
                size="sm"
                type="button"
              >
                <Plus className="size-4" aria-hidden="true" />
                {t("panes.routing.addRule")}
              </Button>
            </PageHeaderActions>
          </PageHeader>
          {/* Radix's intrinsic-width wrapper must not expand the panel to the
              table's minimum width; the table keeps its own horizontal scroll. */}
          <ScrollArea
            className={cn(
              "min-h-0 min-w-0 flex-1 [&_[data-slot=scroll-area-viewport]>div]:block! [&_[data-slot=scroll-area-viewport]>div]:h-full",
              controller.rules.length > 0 && "bg-surface-sunken",
            )}
          >
            <RulesBody controller={controller} processRulesSupported={processRulesSupported} />
          </ScrollArea>
        </PageSurface>
      </PageContent>

      <RoutingRuleDialog
        key={
          ruleDialog?.mode === "edit"
            ? `rule-${ruleDialog.rule.id}`
            : `rule-${ruleDialog?.mode ?? "closed"}`
        }
        mode={ruleDialog?.mode ?? "create"}
        nodeNames={controller.nodeNames}
        onOpenChange={(open) => {
          if (!open) controller.setRuleDialog(null);
        }}
        onSubmit={controller.saveRule}
        open={ruleDialog !== null}
        processRulesSupported={processRulesSupported}
        rule={ruleDialog?.mode === "edit" ? ruleDialog.rule : null}
      />
      <ConfirmDialog controller={controller} />
      {perAppOpen && processRulesSupported ? <PerAppProxyDialog onOpenChange={controller.setPerAppOpen} open /> : null}
    </PageSection>
  );
}

function RulesBody({
  controller,
  processRulesSupported,
}: {
  controller: RoutingScreenController;
  processRulesSupported: boolean;
}) {
  const { t } = useI18n();
  if (!controller.activeRouting) {
    return controller.loading || controller.loadError ? null : (
      <EmptyState
        className="h-full content-center"
        icon={Route}
        title={t("panes.routing.noActiveRouting")}
      />
    );
  }
  if (controller.rules.length === 0) {
    return (
      <EmptyState
        className="h-full content-center"
        description={t("panes.routing.emptyRulesHint")}
        icon={Route}
        title={t("panes.routing.emptyRules")}
      />
    );
  }

  return (
    <RoutingRuleList
      nodeNames={controller.nodeNames}
      onDelete={controller.requestDeleteRule}
      onEdit={controller.editRule}
      onMove={controller.moveRule}
      onReorder={controller.reorderRule}
      onToggle={(rule, enabled) => void controller.toggleRule(rule, enabled)}
      pendingToggles={controller.pendingToggles}
      processRulesSupported={processRulesSupported}
      rules={controller.rules}
    />
  );
}

function ConfirmDialog({ controller }: { controller: RoutingScreenController }) {
  const { t } = useI18n();
  const { confirmPending, pendingConfirm, setPendingConfirm } = controller;
  // Keeps the last request's wording while the dialog animates closed.
  const [shown, setShown] = useState(pendingConfirm);
  if (pendingConfirm !== null && pendingConfirm !== shown) {
    setShown(pendingConfirm);
  }
  const resetting = shown?.kind === "resetRules";

  return (
    <AlertDialog
      onOpenChange={(open) => {
        if (!open) setPendingConfirm(null);
      }}
      open={pendingConfirm !== null}
    >
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            {resetting ? t("confirm.resetRoutingRulesTitle") : t("confirm.deleteRoutingRuleTitle")}
          </AlertDialogTitle>
          <AlertDialogDescription>
            {shown?.kind === "deleteRule"
              ? t("confirm.deleteRoutingRuleDescription", { name: ruleDisplayName(shown.rule, t) })
              : t("confirm.resetRoutingRulesDescription")}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>{t("confirm.cancel")}</AlertDialogCancel>
          <AlertDialogAction
            className={buttonVariants({ variant: "destructive" })}
            onClick={confirmPending}
          >
            {resetting ? t("confirm.resetRoutingRulesConfirm") : t("confirm.deleteRoutingRuleConfirm")}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
