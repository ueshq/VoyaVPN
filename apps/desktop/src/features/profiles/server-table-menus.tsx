import {
  ArrowDown,
  ArrowUp,
  ChevronsDown,
  ChevronsUp,
  Link,
  MoreHorizontal,
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
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from "@voya/ui/components/context-menu";
import {
  Menubar,
  MenubarMenu,
  MenubarTrigger,
  MenubarContent,
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
import type { NodeMenuController } from "./node-controller-types";

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
  disabled,
  label,
  onCancel,
  onRun,
  running,
}: {
  disabled: boolean;
  label: string;
  onCancel: () => Promise<void>;
  onRun: () => Promise<void>;
  running: boolean;
}) {
  const { t } = useI18n();

  return (
    <Button
      disabled={!running && disabled}
      onClick={() => void (running ? onCancel() : onRun())}
      size="sm"
      title={running ? t("panes.profiles.speedtest.cancelTitle") : label}
      type="button"
      variant="outline"
    >
      {running ? (
        <Square className="size-4" aria-hidden="true" />
      ) : (
        <Zap className="size-4" aria-hidden="true" />
      )}
      {running ? t("panes.profiles.speedtest.stop") : label}
    </Button>
  );
}

export function ProfileRowContextMenu({
  children,
  controller,
  item,
}: {
  children: ReactElement;
  controller: NodeMenuController;
  item: ProfileListEntry;
}) {
  return (
    <ContextMenu modal={false}>
      <ContextMenuTrigger
        asChild
        onContextMenu={() => controller.setSelectedId(item.profile.id)}
      >
        {children}
      </ContextMenuTrigger>
      <ContextMenuContent
        aria-label={controller.t("panes.profiles.menu.actionsFor", {
          name: item.profile.remarks || item.profile.id,
        })}
      >
        <ProfileMenuItems
          controller={controller}
          item={item}
          primitives={CONTEXT_ACTION_PRIMITIVES}
        />
      </ContextMenuContent>
    </ContextMenu>
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
  controller: NodeMenuController;
  item: ProfileListEntry;
}) {
  const label = controller.t("panes.profiles.menu.actionsFor", {
    name: item.profile.remarks || item.profile.id,
  });
  return (
    <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
      <MenubarMenu>
        <MenubarTrigger asChild>
          <Button
            aria-label={label}
            onClick={() => controller.setSelectedId(item.profile.id)}
            size="icon"
            variant="ghost"
          >
            <MoreHorizontal aria-hidden="true" className="size-4" />
          </Button>
        </MenubarTrigger>
        <MenubarContent align="end" aria-label={label}>
          <ProfileMenuItems
            controller={controller}
            item={item}
            primitives={MENUBAR_ACTION_PRIMITIVES}
          />
        </MenubarContent>
      </MenubarMenu>
    </Menubar>
  );
}

function ProfileMenuItems({
  controller,
  item,
  primitives: { Item, Separator, Sub, SubContent, SubTrigger },
}: {
  controller: NodeMenuController;
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
      ) : null}
      <Item
        disabled={speedtestRunning}
        onSelect={() => void handleSpeedtest(target)}
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
}) {
  return (
    <>
      {EXPORT_MENU_ENTRIES.map(({ icon: Icon, labelKey, mode }) => (
        <Item key={labelKey} onSelect={mode === "qr" ? onShowQr : onExport}>
          <Icon className="size-4" aria-hidden="true" />
          {t(labelKey)}
        </Item>
      ))}
    </>
  );
}
