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
  /** Global mode skips every rule, so every action is off. */
  locked: boolean;
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
    // The 32 px trigger is wider than the cell's content box; it overhangs the leading padding.
    <Menubar bare className="justify-end">
      <MenubarMenu>
        <MenubarTrigger asChild disabled={actions.locked}>
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
  actions: { canMoveDown, canMoveUp, locked, onDelete, onEdit, onMove },
  primitives: { Item, Separator },
}: {
  actions: RuleMenuActions;
  primitives: MenuPrimitives;
}) {
  const { t } = useI18n();

  return (
    <>
      <Item disabled={locked} onSelect={onEdit}>
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
      <Item disabled={locked} onSelect={onDelete} variant="destructive">
        <Trash2 aria-hidden="true" className="size-4" />
        {t("actions.delete")}
      </Item>
    </>
  );
}
