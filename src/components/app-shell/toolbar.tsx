import type * as React from "react";
import { MoreHorizontal } from "lucide-react";

import { Menubar, MenubarContent, MenubarMenu, MenubarTrigger } from "@/components/ui/menubar";
import { cn } from "@/lib/utils";

// Shared screen-toolbar vocabulary. Screens compose a `Toolbar` row out of one
// or more `ToolbarGroup` clusters (separated by a hairline rule), spill the
// low-priority actions into a `ToolbarOverflow` "⋯" menu, and mount a
// `BulkActionBar` only while a multi-select is active. The overflow menu reuses
// the existing `Menubar` primitive so we add no new dropdown dependency — the
// Profiles "Columns" menu already follows this pattern.

function Toolbar({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn("flex flex-wrap items-center gap-2", className)}
      data-slot="toolbar"
      role="toolbar"
      {...props}
    />
  );
}

// A logical cluster of related controls. Every group after the first carries a
// leading hairline divider so adjacent clusters read as distinct.
function ToolbarGroup({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "flex items-center gap-1 [&:not(:first-child)]:border-s [&:not(:first-child)]:ps-2",
        className,
      )}
      data-slot="toolbar-group"
      {...props}
    />
  );
}

// Overflow "⋯" menu built on Menubar (no new dependency). Pass MenubarItem /
// MenubarCheckboxItem / MenubarSeparator children. `label` names the trigger for
// assistive tech and the tooltip; `className`/rest props flow to the content.
function ToolbarOverflow({
  align = "end",
  children,
  className,
  label,
  ...props
}: React.ComponentProps<typeof MenubarContent> & { label: string }) {
  return (
    <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
      <MenubarMenu>
        <MenubarTrigger aria-label={label} className="size-8 justify-center p-0" title={label}>
          <MoreHorizontal className="size-4" aria-hidden="true" />
        </MenubarTrigger>
        <MenubarContent align={align} className={className} {...props}>
          {children}
        </MenubarContent>
      </MenubarMenu>
    </Menubar>
  );
}

// The contextual action bar shown only while rows are selected. It reads blue to
// echo the table's blue selection state and spans the content edge-to-edge with
// a bottom rule, stacking under the toolbar like InlinePageError.
function BulkActionBar({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "flex flex-wrap items-center gap-2 border-b bg-accent-blue-light px-4 py-2 text-accent-blue",
        className,
      )}
      data-slot="bulk-action-bar"
      role="toolbar"
      {...props}
    />
  );
}

export { BulkActionBar, Toolbar, ToolbarGroup, ToolbarOverflow };
