import type { ReactElement } from "react";
import {
  ArrowDown,
  ArrowUp,
  ChevronsDown,
  ChevronsUp,
  MoreHorizontal,
  Pencil,
  Trash2,
} from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuTrigger,
} from "@voya/ui/components/context-menu";
import {
  Menubar,
  MenubarContent,
  MenubarMenu,
  MenubarTrigger,
} from "@voya/ui/components/menubar";

import {
  CONTEXT_MENU_PRIMITIVES,
  MENUBAR_PRIMITIVES,
  type MenuPrimitives,
} from "@/components/app-shell/menu-primitives";

import type { RuleMoveAction } from "./use-routing-screen";

export type RuleMenuActions = {
  canMoveDown: boolean;
  canMoveUp: boolean;
  onDelete: () => void;
  onEdit: () => void;
  onMove: (action: RuleMoveAction) => void;
};

export function RuleRowContextMenu({
  actions,
  children,
  label,
}: {
  actions: RuleMenuActions;
  children: ReactElement;
  label: string;
}) {
  return (
    <ContextMenu modal={false}>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent aria-label={label}>
        <RuleMenuItems actions={actions} primitives={CONTEXT_MENU_PRIMITIVES} />
      </ContextMenuContent>
    </ContextMenu>
  );
}

export function RuleRowMenuButton({
  actions,
  label,
}: {
  actions: RuleMenuActions;
  label: string;
}) {
  return (
    <Menubar className="h-auto justify-end border-0 bg-transparent p-0 shadow-none">
      <MenubarMenu>
        <MenubarTrigger asChild>
          <Button aria-label={label} size="icon-sm" type="button" variant="ghost">
            <MoreHorizontal aria-hidden="true" className="size-4" />
          </Button>
        </MenubarTrigger>
        <MenubarContent align="end" aria-label={label}>
          <RuleMenuItems actions={actions} primitives={MENUBAR_PRIMITIVES} />
        </MenubarContent>
      </MenubarMenu>
    </Menubar>
  );
}

function RuleMenuItems({
  actions: { canMoveDown, canMoveUp, onDelete, onEdit, onMove },
  primitives: { Item, Separator },
}: {
  actions: RuleMenuActions;
  primitives: MenuPrimitives;
}) {
  const { t } = useI18n();

  return (
    <>
      <Item onSelect={onEdit}>
        <Pencil aria-hidden="true" className="size-4" />
        {t("actions.edit")}
      </Item>
      <Separator />
      <Item disabled={!canMoveUp} onSelect={() => onMove("top")}>
        <ChevronsUp aria-hidden="true" className="size-4" />
        {t("panes.routing.moveRuleTop")}
      </Item>
      <Item disabled={!canMoveUp} onSelect={() => onMove("up")}>
        <ArrowUp aria-hidden="true" className="size-4" />
        {t("panes.routing.moveRuleUp")}
      </Item>
      <Item disabled={!canMoveDown} onSelect={() => onMove("down")}>
        <ArrowDown aria-hidden="true" className="size-4" />
        {t("panes.routing.moveRuleDown")}
      </Item>
      <Item disabled={!canMoveDown} onSelect={() => onMove("bottom")}>
        <ChevronsDown aria-hidden="true" className="size-4" />
        {t("panes.routing.moveRuleBottom")}
      </Item>
      <Separator />
      <Item onSelect={onDelete} variant="destructive">
        <Trash2 aria-hidden="true" className="size-4" />
        {t("actions.delete")}
      </Item>
    </>
  );
}
