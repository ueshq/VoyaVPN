import type * as React from "react";

import { cn } from "@voya/ui/lib/utils";

/**
 * A few mutually exclusive choices that apply immediately. It keeps plain
 * button-group semantics (`group` + `aria-pressed`) and the exact look of
 * TabsList/TabsTrigger, so page tabs and in-page choices read as one control.
 */
function SegmentedControl({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "inline-flex h-8 w-fit items-center justify-center rounded-md bg-surface-hovered p-0.5 text-muted-foreground",
        className,
      )}
      data-slot="segmented-control"
      role="group"
      {...props}
    />
  );
}

function SegmentedControlItem({
  className,
  pressed,
  type = "button",
  ...props
}: Omit<React.ComponentProps<"button">, "aria-pressed"> & { pressed: boolean }) {
  return (
    <button
      aria-pressed={pressed}
      className={cn(
        "inline-flex h-full flex-1 items-center justify-center gap-1.5 whitespace-nowrap rounded-[5px] border border-transparent px-3 text-sm font-medium text-foreground transition-[color,box-shadow] disabled:pointer-events-none disabled:opacity-50 focus-visible:border-ring focus-visible:outline-1 focus-visible:outline-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 not-aria-pressed:hover:bg-background/50 aria-pressed:bg-background aria-pressed:shadow-sm dark:aria-pressed:border-input dark:aria-pressed:bg-input/30 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        className,
      )}
      data-slot="segmented-control-item"
      type={type}
      {...props}
    />
  );
}

export { SegmentedControl, SegmentedControlItem };
