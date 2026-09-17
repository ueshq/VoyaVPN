import type * as React from "react";
import { MoreHorizontal } from "lucide-react";

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

/**
 * The "⋯" overflow menu every row and panel header ends in: a ghost icon
 * trigger opening an end-aligned Menubar. `contentProps` carries the extras
 * only some surfaces need, like a focus restore when a menu item opens a
 * dialog.
 */
function MoreMenu({
  children,
  className,
  contentProps,
  disabled,
  label,
  title,
  triggerRef,
  triggerSize = "icon-sm",
}: Omit<React.ComponentProps<typeof Menubar>, "bare" | "children"> & {
  children: React.ReactNode;
  contentProps?: Omit<
    React.ComponentProps<typeof MenubarContent>,
    "align" | "children"
  >;
  disabled?: boolean;
  /** Announced on the trigger and, unless `contentProps` overrides it, the menu. */
  label: string;
  title?: string;
  triggerRef?: React.Ref<HTMLButtonElement>;
  /** `icon` on roomy card headers, the default `icon-sm` inside dense rows. */
  triggerSize?: "icon" | "icon-sm";
}) {
  return (
    <Menubar bare className={className}>
      <MenubarMenu>
        <MenubarTrigger asChild disabled={disabled}>
          <Button
            aria-label={label}
            ref={triggerRef}
            size={triggerSize}
            title={title}
            type="button"
            variant="ghost"
          >
            <MoreHorizontal aria-hidden="true" className="size-4" />
          </Button>
        </MenubarTrigger>
        <MenubarContent align="end" aria-label={label} {...contentProps}>
          {children}
        </MenubarContent>
      </MenubarMenu>
    </Menubar>
  );
}

/**
 * Wraps a row so right-clicking it opens the same item body its "⋯" menu
 * shows: pass the body as `content` and the row itself as `children`.
 */
function RowContextMenu({
  children,
  content,
  label,
}: {
  children: React.ReactElement;
  content: React.ReactNode;
  label: string;
}) {
  return (
    <ContextMenu modal={false}>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent aria-label={label}>{content}</ContextMenuContent>
    </ContextMenu>
  );
}

export { MoreMenu, RowContextMenu };
