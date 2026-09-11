import {
  ArrowDown,
  ArrowUp,
  ChevronsDown,
  ChevronsUp,
  Download,
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
  ContextMenuItem,
  ContextMenuSeparator,
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
  MenubarItem,
  MenubarSeparator,
} from "@voya/ui/components/menubar";
import { copyProfiles, moveProfile } from "@/ipc";
import type { ProfileListEntry, SpeedtestTarget } from "@/ipc/bindings";
import type { TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";

import { MOVE_ACTIONS } from "./profile-constants";
import type { ProfileExportKind } from "./server-table-actions";
import type { TranslationFunction as TranslateFn } from "@voya/i18n";
import type { ServerTableController } from "./use-server-table";

// The Menubar and ContextMenu variants of the same list are rendered from one
// descriptor array through an injected item primitive, so each export kind
// is added once for both menus.
type MenuItemProps = {
  children: ReactNode;
  disabled?: boolean;
  onSelect?: () => void;
  title?: string;
  variant?: "default" | "destructive";
};

type MenuItemComponent = (props: MenuItemProps) => ReactNode;
type MenuSeparatorComponent = (props: { className?: string }) => ReactNode;

type MenuPrimitives = {
  Item: MenuItemComponent;
  Separator: MenuSeparatorComponent;
};

const MENUBAR_PRIMITIVES: MenuPrimitives = { Item: MenubarItem, Separator: MenubarSeparator };
const CONTEXT_MENU_PRIMITIVES: MenuPrimitives = { Item: ContextMenuItem, Separator: ContextMenuSeparator };

type ExportMenuEntry =
  | { icon: LucideIcon; kind: ProfileExportKind; labelKey: TranslationKey; mode: "export" | "save" }
  | { icon: LucideIcon; labelKey: TranslationKey; mode: "qr" }
  | { mode: "separator" };

const EXPORT_MENU_ENTRIES: readonly ExportMenuEntry[] = [
  { icon: Link, kind: "shareLinks", labelKey: "panes.profiles.export.shareLinks", mode: "export" },
  { icon: Share2, kind: "shareBase64", labelKey: "panes.profiles.export.shareBase64", mode: "export" },
  { icon: Link, kind: "voyaBundle", labelKey: "panes.profiles.export.voyaBundle", mode: "export" },
  { mode: "separator" },
  { icon: QrCode, labelKey: "panes.profiles.export.showQr", mode: "qr" },
  { icon: Download, kind: "shareLinks", labelKey: "panes.profiles.export.saveShareLinks", mode: "save" },
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
      {running ? <Square className="size-4" aria-hidden="true" /> : <Zap className="size-4" aria-hidden="true" />}
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
  controller: ServerTableController;
  item: ProfileListEntry;
}) {
  return (
    <ContextMenu modal={false}>
      <ContextMenuTrigger asChild onContextMenu={() => controller.selectOnly(item.profile.id)}>
        {children}
      </ContextMenuTrigger>
      <ContextMenuContent aria-label={controller.t("panes.profiles.menu.actionsFor", { name: item.profile.remarks || item.profile.id })}>
        <ProfileMenuItems controller={controller} item={item} primitives={CONTEXT_ACTION_PRIMITIVES} />
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
  ...CONTEXT_MENU_PRIMITIVES, Sub: ContextMenuSub, SubTrigger: ContextMenuSubTrigger, SubContent: ContextMenuSubContent,
};
const MENUBAR_ACTION_PRIMITIVES: ActionMenuPrimitives = {
  ...MENUBAR_PRIMITIVES, Sub: MenubarSub, SubTrigger: MenubarSubTrigger, SubContent: MenubarSubContent,
};

export function ProfileCardMenu({ controller, item }: { controller: ServerTableController; item: ProfileListEntry }) {
  const label = controller.t("panes.profiles.menu.actionsFor", { name: item.profile.remarks || item.profile.id });
  return (
    <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
      <MenubarMenu>
        <MenubarTrigger asChild>
          <Button aria-label={label} onClick={() => controller.selectOnly(item.profile.id)} size="icon" variant="ghost">
            <MoreHorizontal aria-hidden="true" className="size-4" />
          </Button>
        </MenubarTrigger>
        <MenubarContent align="end" aria-label={label}>
          <ProfileMenuItems controller={controller} item={item} primitives={MENUBAR_ACTION_PRIMITIVES} />
        </MenubarContent>
      </MenubarMenu>
    </Menubar>
  );
}

function ProfileMenuItems({ controller, item, primitives: { Item, Separator, Sub, SubContent, SubTrigger } }: {
  controller: ServerTableController;
  item: ProfileListEntry;
  primitives: ActionMenuPrimitives;
}) {
  const { handleExport, handleSpeedtest, requestDelete, runOperation, setDialogState, speedtestRunning, t } = controller;
  const indexId = item.profile.id;
  const target: SpeedtestTarget = { scope: "profiles", profileIds: [indexId] };
  return (
    <>
      <Item onSelect={() => setDialogState({ mode: "edit", profile: item })}>
        <Pencil className="size-4" aria-hidden="true" />
        {t("panes.profiles.toolbar.edit")}
      </Item>
      <Item onSelect={() => void runOperation(() => copyProfiles([indexId]))}>{t("nodeGroups.copyNode")}</Item>
      <Sub>
        <SubTrigger>{t("nodeGroups.moveTo")}</SubTrigger>
        <SubContent>
          <Item disabled={controller.nodeGroups.busy} onSelect={() => void controller.nodeGroups.assign([{ profileId: indexId, groupId: null }])}>{t("nodeGroups.unassigned")}</Item>
          {controller.nodeGroups.snapshot.groups.map((group) => <Item key={group.id} disabled={controller.nodeGroups.busy} onSelect={() => void controller.nodeGroups.assign([{ profileId: indexId, groupId: group.id }])}>{group.name}</Item>)}
        </SubContent>
      </Sub>
      <Separator />
      <Item disabled={speedtestRunning} onSelect={() => void handleSpeedtest(target)}>
        <Zap className="size-4" aria-hidden="true" />
        {t("panes.profiles.menu.speedtest")}
      </Item>
      <Sub>
        <SubTrigger>
          <ArrowDown className="size-4" aria-hidden="true" />
          {t("panes.profiles.menu.move")}
        </SubTrigger>
        <SubContent>
          <Item onSelect={() => void runOperation(() => moveProfile(null, indexId, MOVE_ACTIONS.Top, null))}>
            <ChevronsUp className="size-4" aria-hidden="true" />
            {t("panes.profiles.menu.moveTop")}
          </Item>
          <Item onSelect={() => void runOperation(() => moveProfile(null, indexId, MOVE_ACTIONS.Up, null))}>
            <ArrowUp className="size-4" aria-hidden="true" />
            {t("panes.profiles.menu.moveUp")}
          </Item>
          <Item onSelect={() => void runOperation(() => moveProfile(null, indexId, MOVE_ACTIONS.Down, null))}>
            <ArrowDown className="size-4" aria-hidden="true" />
            {t("panes.profiles.menu.moveDown")}
          </Item>
          <Item onSelect={() => void runOperation(() => moveProfile(null, indexId, MOVE_ACTIONS.Bottom, null))}>
            <ChevronsDown className="size-4" aria-hidden="true" />
            {t("panes.profiles.menu.moveBottom")}
          </Item>
        </SubContent>
      </Sub>
      <Sub>
        <SubTrigger>
          <Share2 className="size-4" aria-hidden="true" />
          {t("panes.profiles.export.export")}
        </SubTrigger>
        <SubContent>
          <ExportMenuItems
            onExport={(kind) => void handleExport(kind, [indexId])}
            onSave={(kind) => void handleExport(kind, [indexId], false, true)}
            onShowQr={() => void handleExport("shareLinks", [indexId], true)}
            primitives={{ Item, Separator }}
            t={t}
          />
        </SubContent>
      </Sub>
      <Separator />
      <Item onSelect={() => requestDelete([indexId])} variant="destructive">
        <Trash2 className="size-4" aria-hidden="true" />
        {t("panes.profiles.toolbar.delete")}
      </Item>
    </>
  );
}

export function ExportMenuItems({
  onExport,
  onSave,
  onShowQr,
  // The toolbar renders inside a Menubar; the row menu passes the ContextMenu
  // primitives so both surfaces map the same descriptor list.
  primitives: { Item, Separator } = MENUBAR_PRIMITIVES,
  t,
}: {
  onExport: (kind: ProfileExportKind) => void;
  onSave: (kind: ProfileExportKind) => void;
  onShowQr: () => void;
  primitives?: MenuPrimitives;
  t: TranslateFn;
}) {
  return (
    <>
      {EXPORT_MENU_ENTRIES.map((entry, index) => {
        if (entry.mode === "separator") {
          return <Separator key={`export-separator-${index}`} />;
        }

        const Icon = entry.icon;
        const onSelect =
          entry.mode === "qr"
            ? onShowQr
            : entry.mode === "save"
              ? () => onSave(entry.kind)
              : () => onExport(entry.kind);

        return (
          <Item key={entry.labelKey} onSelect={onSelect}>
            <Icon className="size-4" aria-hidden="true" />
            {t(entry.labelKey)}
          </Item>
        );
      })}
    </>
  );
}
