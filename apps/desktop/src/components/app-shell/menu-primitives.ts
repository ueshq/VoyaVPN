import type { ReactNode } from "react";

import { ContextMenuItem, ContextMenuSeparator } from "@voya/ui/components/context-menu";
import { MenubarItem, MenubarSeparator } from "@voya/ui/components/menubar";

/**
 * The item primitives a menu body is rendered with. A row's "…" Menubar and its
 * right-click ContextMenu render one item list through these, so every action
 * is written once for both menus.
 */
export type MenuPrimitives = {
  Item: (props: {
    children: ReactNode;
    disabled?: boolean;
    onSelect?: () => void;
    title?: string;
    variant?: "default" | "destructive";
  }) => ReactNode;
  Separator: (props: { className?: string }) => ReactNode;
};

export const MENUBAR_PRIMITIVES: MenuPrimitives = {
  Item: MenubarItem,
  Separator: MenubarSeparator,
};

export const CONTEXT_MENU_PRIMITIVES: MenuPrimitives = {
  Item: ContextMenuItem,
  Separator: ContextMenuSeparator,
};
