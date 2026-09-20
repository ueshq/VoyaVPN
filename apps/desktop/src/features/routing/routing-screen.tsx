import { useState } from "react";
import { MoreHorizontal, Plus, RotateCcw, Route } from "lucide-react";
import { Menubar, MenubarMenu, MenubarTrigger, MenubarContent, MenubarItem } from "@voya/ui/components/menubar";

import type { TranslationFunction } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { ConfirmDialog } from "@voya/ui/components/confirm-dialog";
import { Button } from "@voya/ui/components/button";
import { EmptyState } from "@voya/ui/components/empty-state";
import { ScrollArea } from "@voya/ui/components/scroll-area";
import { Skeleton } from "@voya/ui/components/skeleton";
import { cn } from "@voya/ui/lib/utils";

import { InlinePageError } from "@/components/app-shell/inline-page-error";
import {
  PageContent,
  PageHeader,
  PageSection,
  PageSurface,
  PageTitle,
} from "@/components/app-shell/page-section";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

import { PerAppProxyDialog } from "./per-app-proxy-dialog";
import { PerAppSummaryCard } from "./per-app-summary-card";
import { RoutingRuleDialog } from "./routing-rule-dialog";
import { RoutingRuleList } from "./routing-rule-list";
import { ruleDisplayName } from "@voya/features/routing/sentinel-rules";
import { TrafficModeBanner } from "./traffic-mode-banner";
import { TrafficModeSwitcher } from "./traffic-mode-switcher";
import { useProcessRulesSupported } from "@voya/features/routing/use-process-rules-supported";
import { useRoutingScreen, type RoutingScreenController } from "@voya/features/routing/use-routing-screen";
import { useSavedTrafficMode } from "@voya/features/routing/use-traffic-mode";

export function RoutingScreen() {
  const { t } = useI18n();
  const controller = useRoutingScreen();
  const processRulesSupported = useProcessRulesSupported();
  // Rules apply by restarting the core, which drops connections for a moment.
  const connected = useRuntimeEventStore((state) => state.coreState?.state === "connected");
  // Global mode routes all captured traffic ahead of every rule, so none of
  // them can apply and the page locks them.
  const rulesLocked = useSavedTrafficMode().mode === "global";
  const { activeRouting, perAppOpen, ruleDialog } = controller;
  const error = controller.operationError ?? controller.loadError;
  // A disabled button shows no tooltip of its own, so its wrapper says why.
  const editBlockedReason = rulesLocked
    ? t("panes.routing.rulesLocked")
    : activeRouting
      ? undefined
      : t("panes.routing.noActiveRouting");
  const resetLabel = t("panes.routing.resetRules");

  return (
    <PageSection aria-label={t("tabs.rules")}>
      <PageTitle
        // Restore is a named secondary action, separate from the mode switch.
        actions={
          <>
            <TrafficModeSwitcher />
            <Menubar bare>
              <MenubarMenu>
                <MenubarTrigger asChild className="min-h-8">
                  <Button size="sm" variant="outline"><MoreHorizontal aria-hidden="true" className="size-4" />{t("common.more")}</Button>
                </MenubarTrigger>
                <MenubarContent align="end">
                  <MenubarItem disabled={editBlockedReason !== undefined} onSelect={controller.requestResetRules} title={editBlockedReason}>
                    <RotateCcw aria-hidden="true" className="size-4" />{resetLabel}
                  </MenubarItem>
                </MenubarContent>
              </MenubarMenu>
            </Menubar>
            <span title={editBlockedReason}>
              <Button
                disabled={editBlockedReason !== undefined}
                onClick={controller.openCreateRule}
                size="sm"
                type="button"
              >
                <Plus className="size-4" aria-hidden="true" />
                {t("panes.routing.addRule")}
              </Button>
            </span>
          </>
        }
        title={t("tabs.rules")}
      />
      <PageContent>
        {error ? <InlinePageError>{error}</InlinePageError> : null}
        <TrafficModeBanner />
        <PerAppSummaryCard
          locked={rulesLocked}
          onEdit={() => controller.setPerAppOpen(true)}
          routing={activeRouting}
        />
        <PageSurface className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <PageHeader>
            <div className="grid min-w-0 flex-1 gap-0.5">
              <p className="text-sm text-muted-foreground">{t("panes.routing.ruleOrderHint")}</p>
              {connected ? (
                <p className="text-xs text-muted-foreground">{t("panes.routing.reconnectHint")}</p>
              ) : null}
            </div>
          </PageHeader>
          {/* Radix's intrinsic-width wrapper must not expand the panel to the
              table's minimum width; the table keeps its own horizontal scroll. */}
          <ScrollArea
            className={cn(
              "min-h-0 min-w-0 flex-1 [&_[data-slot=scroll-area-viewport]>div]:block! [&_[data-slot=scroll-area-viewport]>div]:h-full",
            )}
          >
            <RulesBody
              controller={controller}
              locked={rulesLocked}
              processRulesSupported={processRulesSupported}
            />
          </ScrollArea>
        </PageSurface>
      </PageContent>

      <RoutingRuleDialog
        key={
          ruleDialog?.mode === "edit"
            ? `rule-${ruleDialog.rule.id}`
            : `rule-${ruleDialog?.mode ?? "closed"}`
        }
        groupOutbounds={controller.groupOutbounds}
        mode={ruleDialog?.mode ?? "create"}
        nodeNames={controller.nodeNames}
        onOpenChange={(open) => {
          if (!open) controller.setRuleDialog(null);
        }}
        onSubmit={controller.saveRule}
        open={ruleDialog !== null}
        processRulesSupported={processRulesSupported}
        rule={ruleDialog?.mode === "edit" ? ruleDialog.rule : null}
        submitError={controller.ruleSaveFailure}
      />
      <PendingConfirmDialog controller={controller} />
      {perAppOpen && processRulesSupported ? <PerAppProxyDialog onOpenChange={controller.setPerAppOpen} open /> : null}
    </PageSection>
  );
}

function RulesBody({
  controller,
  locked,
  processRulesSupported,
}: {
  controller: RoutingScreenController;
  locked: boolean;
  processRulesSupported: boolean;
}) {
  const { t } = useI18n();
  if (controller.loading) {
    // Placeholder rows keep the panel from looking empty while rules load.
    return (
      <div aria-label={t("panes.routing.loadingRules")} className="grid gap-2 p-4" role="status">
        {[0, 1, 2, 3].map((row) => (
          <Skeleton className="h-10 w-full" key={row} />
        ))}
      </div>
    );
  }
  if (!controller.activeRouting) {
    return controller.loadError ? null : (
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
        actions={<Button disabled={locked} onClick={controller.requestResetRules} size="sm" variant="outline">{t("panes.routing.resetRules")}</Button>}
      />
    );
  }

  return (
    <RoutingRuleList
      groupOutbounds={controller.groupOutbounds}
      locked={locked}
      nodeNames={controller.nodeNames}
      onFixOutbound={controller.requestFixOutbound}
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

function PendingConfirmDialog({ controller }: { controller: RoutingScreenController }) {
  const { t } = useI18n();
  const { confirmPending, pendingConfirm, setPendingConfirm } = controller;
  // Keeps the last request's wording while the dialog animates closed.
  const [shown, setShown] = useState(pendingConfirm);
  if (pendingConfirm !== null && pendingConfirm !== shown) {
    setShown(pendingConfirm);
  }
  const copy = confirmCopy(shown, t);

  return (
    <ConfirmDialog
      cancelLabel={t("confirm.cancel")}
      confirmLabel={copy.confirm}
      description={copy.description}
      destructive={copy.destructive}
      onConfirm={confirmPending}
      onOpenChange={(open) => {
        if (!open) setPendingConfirm(null);
      }}
      open={pendingConfirm !== null}
      title={copy.title}
    />
  );
}

function confirmCopy(pending: RoutingScreenController["pendingConfirm"], t: TranslationFunction) {
  switch (pending?.kind) {
    case "fixOutbound":
      // Repairing a rule loses nothing, so it is not styled as destructive.
      return {
        confirm: t("confirm.fixOutboundConfirm"),
        description: t("confirm.fixOutboundDescription"),
        destructive: false,
        title: t("confirm.fixOutboundTitle", { name: ruleDisplayName(pending.rule, t) }),
      };
    case "resetRules":
      return {
        confirm: t("confirm.resetRoutingRulesConfirm"),
        description: t("confirm.resetRoutingRulesDescription"),
        destructive: true,
        title: t("confirm.resetRoutingRulesTitle"),
      };
    default:
      return {
        confirm: t("confirm.deleteRoutingRuleConfirm"),
        description: pending
          ? t("confirm.deleteRoutingRuleDescription", { name: ruleDisplayName(pending.rule, t) })
          : "",
        destructive: true,
        title: t("confirm.deleteRoutingRuleTitle"),
      };
  }
}
