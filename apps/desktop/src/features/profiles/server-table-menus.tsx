import {
  Activity,
  ArrowDown,
  ArrowUp,
  ChevronDown,
  ChevronsDown,
  ChevronsUp,
  Clock,
  Download,
  FileJson2,
  Gauge,
  Link,
  Pencil,
  QrCode,
  Radio,
  Share2,
  Square,
  Trash2,
  Wifi,
  Zap,
} from "lucide-react";
import type { ReactElement, ReactNode } from "react";
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
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarTrigger,
} from "@voya/ui/components/menubar";
import { moveProfile } from "@/ipc";
import type { ProfileListEntry, SpeedtestKind, SpeedtestTarget } from "@/ipc/bindings";
import type { TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";

import { MOVE_ACTIONS, SPEED_ACTIONS } from "./profile-constants";
import type { ProfileExportKind } from "./server-table-actions";
import type { TranslateFn } from "./server-table-columns";
import type { ServerTableController } from "./use-server-table";

// The Menubar and ContextMenu variants of the same list are rendered from one
// descriptor array through an injected item primitive, so a new export kind or
// probe mode is added once instead of two or three times (which is how the
// duplicated Fast/Real latency entry went unnoticed).
type MenuItemProps = {
  children: ReactNode;
  disabled?: boolean;
  onSelect?: () => void;
  title?: string;
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
  { icon: FileJson2, kind: "clientConfig", labelKey: "panes.profiles.export.clientConfig", mode: "export" },
  { mode: "separator" },
  { icon: QrCode, labelKey: "panes.profiles.export.showQr", mode: "qr" },
  { icon: Download, kind: "shareLinks", labelKey: "panes.profiles.export.saveShareLinks", mode: "save" },
  { icon: FileJson2, kind: "clientConfig", labelKey: "panes.profiles.export.saveClientConfig", mode: "save" },
];

// `latency` is the real-delay probe; there is no separate cheap "fast" kind in
// `SpeedtestKind`, so the menu offers each probe exactly once.
const SPEED_MENU_ENTRIES: ReadonlyArray<{ action: SpeedtestKind; icon: LucideIcon; labelKey: TranslationKey }> = [
  { action: SPEED_ACTIONS.TcpConnect, icon: Activity, labelKey: "panes.profiles.speedtest.tcp" },
  { action: SPEED_ACTIONS.Latency, icon: Clock, labelKey: "panes.profiles.speedtest.real" },
  { action: SPEED_ACTIONS.Udp, icon: Radio, labelKey: "panes.profiles.speedtest.udp" },
  { action: SPEED_ACTIONS.Download, icon: Gauge, labelKey: "panes.profiles.speedtest.speed" },
  { action: SPEED_ACTIONS.Mixed, icon: Wifi, labelKey: "panes.profiles.speedtest.mixed" },
];

// Speedtest split button: the default real-delay probe runs straight from the
// primary control, while the chevron opens a menu for every probe mode plus the
// running-only Stop. The dropdown reuses the Menubar primitive (no new
// dependency) so its trigger and items expose `menuitem` roles, mirroring the
// Columns menu.
export function SpeedtestSplitButton({
  disabled,
  label,
  onCancel,
  onRun,
  running,
}: {
  disabled: boolean;
  label: string;
  onCancel: () => Promise<void>;
  onRun: (kind: SpeedtestKind) => Promise<void>;
  running: boolean;
}) {
  const { t } = useI18n();

  return (
    <div className="flex items-center">
      <Button
        className="rounded-e-none"
        disabled={disabled || running}
        onClick={() => void onRun(SPEED_ACTIONS.Latency)}
        size="sm"
        title={t("panes.profiles.speedtest.buttonTitle", { label })}
        type="button"
        variant="outline"
      >
        <Zap className="size-4" aria-hidden="true" />
        {label}
      </Button>
      <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
        <MenubarMenu>
          <MenubarTrigger asChild>
            <Button
              aria-label={t("panes.profiles.speedtest.more")}
              className="rounded-s-none border-s-0 px-2"
              disabled={disabled}
              size="sm"
              title={t("panes.profiles.speedtest.more")}
              type="button"
              variant="outline"
            >
              <ChevronDown className="size-4" aria-hidden="true" />
            </Button>
          </MenubarTrigger>
          <MenubarContent align="start">
            <SpeedMenuItems
              onRun={onRun}
              primitives={MENUBAR_PRIMITIVES}
              running={running}
              t={t}
            />
            <MenubarSeparator />
            <MenubarItem
              disabled={!running}
              onSelect={() => void onCancel()}
              title={t("panes.profiles.speedtest.cancelTitle")}
            >
              <Square className="size-4" aria-hidden="true" />
              {t("panes.profiles.speedtest.stop")}
            </MenubarItem>
          </MenubarContent>
        </MenubarMenu>
      </Menubar>
    </div>
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
  const {
    handleCancelSpeedtest,
    handleExport,
    handleSpeedtest,
    requestDelete,
    runOperation,
    selectOnly,
    setDialogState,
    speedtestRunning,
    t,
  } = controller;
  const indexId = item.profile.id;
  const target: SpeedtestTarget = { scope: "profiles", profileIds: [indexId] };
  const runTargetSpeedtest = (kind: SpeedtestKind) => handleSpeedtest(kind, target);

  return (
    // A row action can open a modal dialog. Keeping the short-lived context
    // menu non-modal avoids competing focus scopes while the menu closes.
    <ContextMenu modal={false}>
      <ContextMenuTrigger asChild onContextMenu={() => selectOnly(indexId)}>
        {children}
      </ContextMenuTrigger>
      <ContextMenuContent
        aria-label={t("panes.profiles.menu.actionsFor", {
          name: item.profile.remarks || indexId,
        })}
      >
        <ContextMenuItem onSelect={() => setDialogState({ mode: "edit", profile: item })}>
          <Pencil className="size-4" aria-hidden="true" />
          {t("panes.profiles.toolbar.edit")}
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuSub>
          <ContextMenuSubTrigger>
            <Zap className="size-4" aria-hidden="true" />
            {t("panes.profiles.menu.speedtest")}
          </ContextMenuSubTrigger>
          <ContextMenuSubContent>
            <SpeedMenuItems
              onRun={runTargetSpeedtest}
              primitives={CONTEXT_MENU_PRIMITIVES}
              running={speedtestRunning}
              t={t}
            />
            <ContextMenuSeparator />
            <ContextMenuItem
              disabled={!speedtestRunning}
              onSelect={() => void handleCancelSpeedtest()}
              title={t("panes.profiles.speedtest.cancelTitle")}
            >
              <Square className="size-4" aria-hidden="true" />
              {t("panes.profiles.speedtest.stop")}
            </ContextMenuItem>
          </ContextMenuSubContent>
        </ContextMenuSub>
        <ContextMenuSub>
          <ContextMenuSubTrigger>
            <ArrowDown className="size-4" aria-hidden="true" />
            {t("panes.profiles.menu.move")}
          </ContextMenuSubTrigger>
          <ContextMenuSubContent>
            <ContextMenuItem onSelect={() => void runOperation(() => moveProfile(null, indexId, MOVE_ACTIONS.Top, null))}>
              <ChevronsUp className="size-4" aria-hidden="true" />
              {t("panes.profiles.menu.moveTop")}
            </ContextMenuItem>
            <ContextMenuItem onSelect={() => void runOperation(() => moveProfile(null, indexId, MOVE_ACTIONS.Up, null))}>
              <ArrowUp className="size-4" aria-hidden="true" />
              {t("panes.profiles.menu.moveUp")}
            </ContextMenuItem>
            <ContextMenuItem onSelect={() => void runOperation(() => moveProfile(null, indexId, MOVE_ACTIONS.Down, null))}>
              <ArrowDown className="size-4" aria-hidden="true" />
              {t("panes.profiles.menu.moveDown")}
            </ContextMenuItem>
            <ContextMenuItem onSelect={() => void runOperation(() => moveProfile(null, indexId, MOVE_ACTIONS.Bottom, null))}>
              <ChevronsDown className="size-4" aria-hidden="true" />
              {t("panes.profiles.menu.moveBottom")}
            </ContextMenuItem>
          </ContextMenuSubContent>
        </ContextMenuSub>
        <ContextMenuSub>
          <ContextMenuSubTrigger>
            <Share2 className="size-4" aria-hidden="true" />
            {t("panes.profiles.export.export")}
          </ContextMenuSubTrigger>
          <ContextMenuSubContent>
            <ExportMenuItems
              onExport={(kind) => void handleExport(kind, [indexId])}
              onSave={(kind) => void handleExport(kind, [indexId], false, true)}
              onShowQr={() => void handleExport("shareLinks", [indexId], true)}
              primitives={CONTEXT_MENU_PRIMITIVES}
              t={t}
            />
          </ContextMenuSubContent>
        </ContextMenuSub>
        <ContextMenuSeparator />
        <ContextMenuItem onSelect={() => requestDelete([indexId])} variant="destructive">
          <Trash2 className="size-4" aria-hidden="true" />
          {t("panes.profiles.toolbar.delete")}
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
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

function SpeedMenuItems({
  onRun,
  primitives: { Item } = MENUBAR_PRIMITIVES,
  running,
  t,
}: {
  onRun: (kind: SpeedtestKind) => Promise<void>;
  primitives?: MenuPrimitives;
  running: boolean;
  t: TranslateFn;
}) {
  return (
    <>
      {SPEED_MENU_ENTRIES.map((entry) => {
        const Icon = entry.icon;

        return (
          <Item disabled={running} key={entry.labelKey} onSelect={() => void onRun(entry.action)}>
            <Icon className="size-4" aria-hidden="true" />
            {t(entry.labelKey)}
          </Item>
        );
      })}
    </>
  );
}
