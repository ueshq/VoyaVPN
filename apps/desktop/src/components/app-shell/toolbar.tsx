import type * as React from "react";

import { cn } from "@voya/ui/lib/utils";

// Shared screen-toolbar vocabulary. Screens compose a `Toolbar` row out of one
// or more `ToolbarGroup` clusters separated by a hairline rule.

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

export { Toolbar, ToolbarGroup };
