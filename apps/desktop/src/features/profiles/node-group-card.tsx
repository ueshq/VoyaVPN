import { useRef } from "react";
import {
  ChevronDown,
  ChevronRight,
  Folder,
  Rss,
  RefreshCw,
  Settings,
  Trash2,
  MoreHorizontal,
  Pencil,
  Share2,
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
import type { ServerTableController } from "./use-server-table";

export function NodeGroupCard({
  row,
  controller,
}: {
  row: Extract<NodeListRow, { kind: "group" }>;
  controller: ServerTableController;
}) {
  const {
    nodeGroups,
    t,
    handleGroupExport,
    handleSpeedtest,
    handleCancelSpeedtest,
    speedtestRunning,
  } = controller;
  const group = row.group;
  const subscription = row.subscription;
  const { language } = useI18n();
  const metadata = subscription
    ? controller.subscriptionMetadata.get(subscription.id)
    : null;
  const trigger = useRef<HTMLButtonElement>(null);
  const opening = useRef(false);
  const position = nodeGroups.snapshot.groups.findIndex(
    (g) => g.id === group?.id,
  );
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
            </span>
          </span>
        </button>
        <div className="node-group-actions">
          {subscription ? (
            <>
              <Button
                size="sm"
                variant="outline"
                disabled={controller.updatingSubscriptions.has(subscription.id)}
                onClick={() =>
                  void controller.updateSubscription(subscription.id)
                }
              >
                <RefreshCw
                  aria-hidden="true"
                  className={
                    controller.updatingSubscriptions.has(subscription.id)
                      ? "size-4 animate-spin"
                      : "size-4"
                  }
                />
                {t("home.subscriptionCard.update")}
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={(event) =>
                  controller.openSubscription(subscription, event.currentTarget)
                }
              >
                <Settings aria-hidden="true" className="size-4" />
                {t("subscriptions.edit")}
              </Button>
              <Button
                size="icon"
                variant="ghost"
                aria-label={t("subscriptions.deleteTitle", { name: row.name })}
                onClick={(event) => {
                  controller.confirmSubscriptionDeletion(
                    subscription,
                    event.currentTarget,
                  );
                }}
              >
                <Trash2 aria-hidden="true" className="size-4" />
              </Button>
            </>
          ) : null}
          <SpeedtestButton
            disabled={!row.members.length}
            label={t("nodeGroups.test")}
            onCancel={handleCancelSpeedtest}
            onRun={() =>
              handleSpeedtest({
                scope: "profiles",
                profileIds: row.members.map((p) => p.profile.id),
              })
            }
            running={speedtestRunning}
          />
          {group ? (
            <Button
              disabled={nodeGroups.busy}
              size="sm"
              variant="outline"
              onClick={(event) =>
                nodeGroups.open({ kind: "edit", group }, event.currentTarget)
              }
            >
              <Pencil aria-hidden="true" className="size-4" />
              {t("nodeGroups.edit")}
            </Button>
          ) : null}
          <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
            <MenubarMenu>
              <MenubarTrigger asChild>
                <Button
                  aria-label={t("nodeGroups.exportFor", { name: row.name })}
                  disabled={!row.members.length}
                  size="sm"
                  variant="outline"
                >
                  <Share2 aria-hidden="true" className="size-4" />
                  {t("panes.profiles.export.export")}
                </Button>
              </MenubarTrigger>
              <MenubarContent align="end">
                <ExportMenuItems
                  t={t}
                  onExport={(kind) =>
                    void handleGroupExport(row.groupKey, kind)
                  }
                  onSave={(kind) =>
                    void handleGroupExport(row.groupKey, kind, false, true)
                  }
                  onShowQr={() =>
                    void handleGroupExport(row.groupKey, "shareLinks", true)
                  }
                />
              </MenubarContent>
            </MenubarMenu>
          </Menubar>
          {group ? (
            <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
              <MenubarMenu>
                <MenubarTrigger asChild>
                  <Button
                    ref={trigger}
                    aria-label={t("nodeGroups.actions", { name: group.name })}
                    disabled={nodeGroups.busy}
                    size="icon"
                    variant="ghost"
                  >
                    <MoreHorizontal aria-hidden="true" className="size-4" />
                  </Button>
                </MenubarTrigger>
                <MenubarContent
                  align="end"
                  onCloseAutoFocus={(event) => {
                    if (opening.current) {
                      event.preventDefault();
                      opening.current = false;
                    }
                  }}
                >
                  <MenubarItem
                    disabled={position <= 0}
                    onSelect={() => void nodeGroups.move(group.id, "up")}
                  >
                    {t("panes.profiles.menu.moveUp")}
                  </MenubarItem>
                  <MenubarItem
                    disabled={
                      position === nodeGroups.snapshot.groups.length - 1
                    }
                    onSelect={() => void nodeGroups.move(group.id, "down")}
                  >
                    {t("panes.profiles.menu.moveDown")}
                  </MenubarItem>
                  <MenubarSeparator />
                  <MenubarItem
                    variant="destructive"
                    onSelect={() => {
                      opening.current = true;
                      nodeGroups.open(
                        { kind: "delete", group },
                        trigger.current,
                      );
                    }}
                  >
                    {t("nodeGroups.delete")}
                  </MenubarItem>
                </MenubarContent>
              </MenubarMenu>
            </Menubar>
          ) : null}
        </div>
      </div>
      {subscription ? (
        <SubscriptionMetaLine
          language={language}
          metadata={metadata}
          t={t}
          className="px-4 pb-3"
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
