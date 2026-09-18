import {
  ArrowDown,
  ArrowUp,
  ChevronsDown,
  ChevronsUp,
  Info,
  Link,
  Pencil,
  QrCode,
  Share2,
  Square,
  Trash2,
  Zap,
} from "lucide-react";
import type { ComponentType, ReactElement, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { MoreMenu, RowContextMenu } from "@voya/ui/components/row-menus";
import { DisabledReason } from "@/components/disabled-reason";
import {
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
} from "@voya/ui/components/context-menu";
import {
  MenubarSub,
  MenubarSubTrigger,
  MenubarSubContent,
} from "@voya/ui/components/menubar";
import {
  CONTEXT_MENU_PRIMITIVES,
  MENUBAR_PRIMITIVES,
  type MenuPrimitives,
} from "@/components/app-shell/menu-primitives";
import { moveProfile } from "@/ipc/commands";
import type { ProfileListEntry, SpeedtestTarget } from "@/ipc/bindings";
import type { TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";

import { MOVE_ACTIONS } from "./profile-constants";
import type { TranslationFunction as TranslateFn } from "@voya/i18n";
import type { ServerTableController } from "./use-server-table";

type ExportMenuEntry = {
  icon: LucideIcon;
  labelKey: TranslationKey;
  mode: "export" | "qr";
};

const EXPORT_MENU_ENTRIES: readonly ExportMenuEntry[] = [
  { icon: Link, labelKey: "panes.profiles.export.shareLinks", mode: "export" },
  { icon: QrCode, labelKey: "panes.profiles.export.showQr", mode: "qr" },
];

export function SpeedtestButton({
  busyElsewhere = false,
  disabled,
  label,
  onCancel,
  onRun,
  progress = null,
  running,
  variant = "outline",
}: {
  /** Another entry point started the running test; only that one can stop it. */
  busyElsewhere?: boolean;
  disabled: boolean;
  label: string;
  onCancel: () => Promise<void>;
  onRun: () => Promise<void>;
  progress?: { done: number; total: number } | null;
  running: boolean;
  /** Page toolbars use outline; group headers repeat per group, so they stay quiet. */
  variant?: "outline" | "ghost";
}) {
  const { t } = useI18n();

  return (
    <DisabledReason
      reason={!running && busyElsewhere ? t("panes.profiles.speedtest.runningElsewhere") : undefined}
    >
    <Button
      disabled={!running && (disabled || busyElsewhere)}
      onClick={() => void (running ? onCancel() : onRun())}
      size="sm"
      title={running ? t("panes.profiles.speedtest.cancelTitle") : label}
      type="button"
      variant={variant}
    >
      {running ? (
        <Square className="size-4" aria-hidden="true" />
      ) : (
        <Zap className="size-4" aria-hidden="true" />
      )}
      <span data-slot="button-label">
        {running
          ? progress
            ? t("panes.profiles.speedtest.stopProgress", progress)
            : t("panes.profiles.speedtest.stop")
          : label}
      </span>
    </Button>
    </DisabledReason>
  );
}

export function ProfileRowContextMenu({
  children,
  controller,
  item,
}: {
  children: ReactElement;
  controller: ServerTableController;
  item: ProfileListEntry;
}) {
  return (
    <RowContextMenu
      content={
        <ProfileMenuItems
          controller={controller}
          item={item}
          primitives={CONTEXT_ACTION_PRIMITIVES}
        />
      }
      label={controller.t("panes.profiles.menu.actionsFor", {
        name: item.profile.remarks || item.profile.id,
      })}
    >
      {children}
    </RowContextMenu>
  );
}

type ActionMenuPrimitives = MenuPrimitives & {
  Sub: ComponentType<{ children: ReactNode }>;
  SubTrigger: ComponentType<{ children: ReactNode }>;
  SubContent: ComponentType<{ children: ReactNode }>;
};
const CONTEXT_ACTION_PRIMITIVES: ActionMenuPrimitives = {
  ...CONTEXT_MENU_PRIMITIVES,
  Sub: ContextMenuSub,
  SubTrigger: ContextMenuSubTrigger,
  SubContent: ContextMenuSubContent,
};
const MENUBAR_ACTION_PRIMITIVES: ActionMenuPrimitives = {
  ...MENUBAR_PRIMITIVES,
  Sub: MenubarSub,
  SubTrigger: MenubarSubTrigger,
  SubContent: MenubarSubContent,
};

export function ProfileCardMenu({
  controller,
  item,
}: {
  controller: ServerTableController;
  item: ProfileListEntry;
}) {
  return (
    <MoreMenu
      label={controller.t("panes.profiles.menu.actionsFor", {
        name: item.profile.remarks || item.profile.id,
      })}
      triggerSize="icon"
    >
      <ProfileMenuItems
        controller={controller}
        item={item}
        primitives={MENUBAR_ACTION_PRIMITIVES}
      />
    </MoreMenu>
  );
}

function ProfileMenuItems({
  controller,
  item,
  primitives: { Item, Separator, Sub, SubContent, SubTrigger },
}: {
  controller: ServerTableController;
  item: ProfileListEntry;
  primitives: ActionMenuPrimitives;
}) {
  const {
    handleExport,
    handleSpeedtest,
    requestDelete,
    runOperation,
    setDialogState,
    speedtestRunning,
    t,
  } = controller;
  const indexId = item.profile.id;
  const manual = !item.profile.subscriptionId;
  const target: SpeedtestTarget = { scope: "profiles", profileIds: [indexId] };
  return (
    <>
      {manual ? (
        <>
          <Item
            onSelect={() => setDialogState({ mode: "edit", profile: item })}
          >
            <Pencil className="size-4" aria-hidden="true" />
            {t("panes.profiles.toolbar.edit")}
          </Item>
          <Separator />
        </>
      ) : (
        <>
          {/* Say why edit, move and delete are missing instead of hiding them silently. */}
          <Item disabled>
            <Info className="size-4" aria-hidden="true" />
            {t("panes.profiles.menu.subscriptionReadOnly")}
          </Item>
          <Separator />
        </>
      )}
      <Item
        disabled={speedtestRunning}
        onSelect={() => void handleSpeedtest(target, `node:${indexId}`)}
      >
        <Zap className="size-4" aria-hidden="true" />
        {t("panes.profiles.menu.speedtest")}
      </Item>
      {manual ? (
        <Sub>
          <SubTrigger>
            <ArrowDown className="size-4" aria-hidden="true" />
            {t("panes.profiles.menu.move")}
          </SubTrigger>
          <SubContent>
            <Item
              onSelect={() =>
                void runOperation(() =>
                  moveProfile(null, indexId, MOVE_ACTIONS.Top, null),
                )
              }
            >
              <ChevronsUp className="size-4" aria-hidden="true" />
              {t("panes.profiles.menu.moveTop")}
            </Item>
            <Item
              onSelect={() =>
                void runOperation(() =>
                  moveProfile(null, indexId, MOVE_ACTIONS.Up, null),
                )
              }
            >
              <ArrowUp className="size-4" aria-hidden="true" />
              {t("panes.profiles.menu.moveUp")}
            </Item>
            <Item
              onSelect={() =>
                void runOperation(() =>
                  moveProfile(null, indexId, MOVE_ACTIONS.Down, null),
                )
              }
            >
              <ArrowDown className="size-4" aria-hidden="true" />
              {t("panes.profiles.menu.moveDown")}
            </Item>
            <Item
              onSelect={() =>
                void runOperation(() =>
                  moveProfile(null, indexId, MOVE_ACTIONS.Bottom, null),
                )
              }
            >
              <ChevronsDown className="size-4" aria-hidden="true" />
              {t("panes.profiles.menu.moveBottom")}
            </Item>
          </SubContent>
        </Sub>
      ) : null}
      <Sub>
        <SubTrigger>
          <Share2 className="size-4" aria-hidden="true" />
          {t("panes.profiles.export.export")}
        </SubTrigger>
        <SubContent>
          <ExportMenuItems
            onExport={() => void handleExport([indexId])}
            onShowQr={() => void handleExport([indexId], "qr")}
            primitives={{ Item }}
            t={t}
          />
        </SubContent>
      </Sub>
      {manual ? (
        <>
          <Separator />
          <Item onSelect={() => requestDelete([indexId])} variant="destructive">
            <Trash2 className="size-4" aria-hidden="true" />
            {t("panes.profiles.toolbar.delete")}
          </Item>
        </>
      ) : null}
    </>
  );
}

export function ExportMenuItems({
  disabled = false,
  onExport,
  onShowQr,
  // The group card renders inside a Menubar; the row menu passes its own item
  // primitive so both surfaces map the same descriptor list.
  primitives: { Item } = MENUBAR_PRIMITIVES,
  t,
}: {
  onExport: () => void;
  onShowQr: () => void;
  primitives?: Pick<MenuPrimitives, "Item">;
  t: TranslateFn;
  /** Nothing to export, such as a group without nodes. */
  disabled?: boolean;
}) {
  return (
    <>
      {EXPORT_MENU_ENTRIES.map(({ icon: Icon, labelKey, mode }) => (
        <Item disabled={disabled} key={labelKey} onSelect={mode === "qr" ? onShowQr : onExport}>
          <Icon className="size-4" aria-hidden="true" />
          {t(labelKey)}
        </Item>
      ))}
    </>
  );
}
