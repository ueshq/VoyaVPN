import { useRef } from "react";
import {
  ChevronDown,
  ChevronRight,
  Folder,
  MoreHorizontal,
  RefreshCw,
  Rss,
  Settings,
  Trash2,
} from "lucide-react";
import { useI18n } from "@voya/i18n/use-i18n";
import { SubscriptionMetaLine } from "@/features/subscriptions/subscription-card";
import { Button } from "@voya/ui/components/button";
import {
  Menubar,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarTrigger,
} from "@voya/ui/components/menubar";
import { ExportMenuItems, SpeedtestButton } from "./server-table-menus";
import type { NodeListRow } from "./node-list-rows";
import type { NodeGroupCardController } from "./node-controller-types";

export function NodeGroupCard({
  row,
  controller,
}: {
  row: Extract<NodeListRow, { kind: "group" }>;
  controller: NodeGroupCardController;
}) {
  const {
    nodeGroups,
    t,
    handleGroupExport,
    handleSpeedtest,
    handleCancelSpeedtest,
    speedtestProgress,
    speedtestRunning,
    speedtestSource,
  } = controller;
  const subscription = row.subscription;
  const { language } = useI18n();
  const moreRef = useRef<HTMLButtonElement>(null);
  // A dialog opened from the menu keeps its focus instead of the menu's trigger.
  const openingDialogRef = useRef(false);
  const metadata = subscription
    ? controller.subscriptionMetadata.get(subscription.id)
    : null;
  const source = `group:${row.groupKey}`;
  const moreLabel = t("nodeGroups.moreFor", { name: row.name });
  const updateLabel = t("home.subscriptionCard.update");
  const returnFocusTo = () => moreRef.current ?? document.body;

  return (
    <article
      className="node-group-surface node-group-header"
      data-testid="node-group-card"
      aria-label={row.name}
    >
      <div className="node-group-heading">
        <button
          className="node-group-toggle"
          aria-label={row.name}
          aria-expanded={row.expanded}
          data-row-focus
          onClick={() => nodeGroups.toggle(row.groupKey)}
          type="button"
        >
          {row.expanded ? (
            <ChevronDown aria-hidden="true" className="size-4 shrink-0" />
          ) : (
            <ChevronRight aria-hidden="true" className="size-4 shrink-0" />
          )}
          {row.groupKey.startsWith("subscription:") ? (
            <Rss
              aria-hidden="true"
              className="size-5 shrink-0 text-muted-foreground"
            />
          ) : (
            <Folder
              aria-hidden="true"
              className="size-5 shrink-0 text-muted-foreground"
            />
          )}
          <span className="min-w-0">
            <span className="block truncate font-semibold" title={row.name}>
              {row.name}
            </span>
            <span className="text-xs text-muted-foreground">
              {t("nodeGroups.membersCount", { count: row.members.length })}
              {subscription && !subscription.enabled
                ? ` · ${t("nodeGroups.autoUpdateOff")}`
                : null}
            </span>
          </span>
        </button>
        {/* The everyday actions stay on the header; the rest sits behind the menu.
            They repeat for every group, so they are quiet ghost buttons. */}
        <div className="node-group-actions">
          {subscription ? (
            <Button
              size="sm"
              variant="ghost"
              disabled={controller.updatingSubscriptions.has(subscription.id)}
              onClick={() => void controller.updateSubscription(subscription.id)}
              title={updateLabel}
            >
              <RefreshCw
                aria-hidden="true"
                className={
                  controller.updatingSubscriptions.has(subscription.id)
                    ? "size-4 animate-spin"
                    : "size-4"
                }
              />
              <span data-slot="button-label">{updateLabel}</span>
            </Button>
          ) : null}
          <SpeedtestButton
            busyElsewhere={speedtestRunning && speedtestSource !== source}
            disabled={!row.allMembers.length}
            label={t("nodeGroups.test")}
            onCancel={handleCancelSpeedtest}
            onRun={() =>
              handleSpeedtest(
                {
                  scope: "profiles",
                  profileIds: row.allMembers.map((p) => p.profile.id),
                },
                source,
              )
            }
            progress={speedtestProgress}
            running={speedtestRunning && speedtestSource === source}
            variant="ghost"
          />
          <Menubar bare>
            <MenubarMenu>
              <MenubarTrigger asChild>
                <Button
                  aria-label={moreLabel}
                  ref={moreRef}
                  size="icon-sm"
                  title={moreLabel}
                  type="button"
                  variant="ghost"
                >
                  <MoreHorizontal aria-hidden="true" className="size-4" />
                </Button>
              </MenubarTrigger>
              <MenubarContent
                align="end"
                aria-label={moreLabel}
                onCloseAutoFocus={(event) => {
                  if (openingDialogRef.current) {
                    event.preventDefault();
                    openingDialogRef.current = false;
                  }
                }}
              >
                {subscription ? (
                  <>
                    <MenubarItem
                      onSelect={() => {
                        openingDialogRef.current = true;
                        controller.openSubscription(subscription, returnFocusTo());
                      }}
                    >
                      <Settings aria-hidden="true" className="size-4" />
                      {t("subscriptions.edit")}
                    </MenubarItem>
                    <MenubarSeparator />
                  </>
                ) : null}
                {/* Hidden unreachable nodes still export, so only an empty group cannot. */}
                <ExportMenuItems
                  disabled={!row.allMembers.length}
                  onExport={() => void handleGroupExport(row.groupKey)}
                  onShowQr={() => void handleGroupExport(row.groupKey, "qr")}
                  t={t}
                />
                {subscription ? (
                  <>
                    <MenubarSeparator />
                    <MenubarItem
                      onSelect={() => {
                        openingDialogRef.current = true;
                        controller.confirmSubscriptionDeletion(subscription, returnFocusTo());
                      }}
                      variant="destructive"
                    >
                      <Trash2 aria-hidden="true" className="size-4" />
                      {t("subscriptions.delete")}
                    </MenubarItem>
                  </>
                ) : null}
              </MenubarContent>
            </MenubarMenu>
          </Menubar>
        </div>
      </div>
      {subscription ? (
        <SubscriptionMetaLine
          language={language}
          metadata={metadata}
          t={t}
          className="px-5 pb-3"
        />
      ) : null}
      {row.expanded && !row.members.length ? (
        <p className="node-group-empty">
          {t(subscription ? "subscriptions.empty" : "nodeGroups.empty")}
        </p>
      ) : null}
    </article>
  );
}
