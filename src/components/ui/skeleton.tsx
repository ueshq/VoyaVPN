import * as React from "react";

import { cn } from "@/lib/utils";

function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return <div data-slot="skeleton" className={cn("animate-pulse rounded-md bg-accent", className)} {...props} />;
}

// Shared loading scaffold for the virtualized data tables (Profiles, Clash
// connections, …) that each used to hand-roll a near-identical placeholder grid.
// Every placeholder row mirrors the real row geometry — the caller's
// `gridTemplateColumns` + `gridMinWidth` + a per-row pixel `rowHeight` — so the
// table does not reflow when the data resolves. Row heights stay prop-driven
// (never hard-coded) because the virtualizer's `estimateSize` and the screen
// tests assert the exact 38px / 40px row heights. The leading cell stands in for
// the row's selection checkbox or status indicator; body cells carry the table's
// vertical rules when `bordered`.
function TableSkeletonRows({
  bordered = true,
  className,
  columnCount,
  gridMinWidth,
  gridTemplateColumns,
  leading = "checkbox",
  rowCount = 8,
  rowHeight = 40,
  ...props
}: React.ComponentProps<"div"> & {
  bordered?: boolean;
  columnCount: number;
  gridMinWidth?: string;
  gridTemplateColumns: string;
  leading?: "checkbox" | "indicator" | "none";
  rowCount?: number;
  rowHeight?: number;
}) {
  return (
    <div className={className} role="status" {...props}>
      {Array.from({ length: rowCount }).map((_, rowIndex) => (
        <div
          className={cn("grid items-center border-b", !bordered && "px-4")}
          key={rowIndex}
          style={{ gridTemplateColumns, height: rowHeight, minWidth: gridMinWidth }}
        >
          {leading === "none" ? null : leading === "indicator" ? (
            <span className="block size-4 rounded-full border bg-background" aria-hidden="true" />
          ) : (
            <div className={cn("flex h-full items-center justify-center px-2", bordered && "border-e")}>
              <Skeleton className="size-4 rounded-sm" />
            </div>
          )}
          {Array.from({ length: columnCount }).map((_, columnIndex) => (
            <div
              className={cn("flex h-full min-w-0 items-center", bordered && "border-e px-2 last:border-e-0")}
              key={columnIndex}
            >
              <Skeleton className="h-4 w-3/4" />
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

export { Skeleton, TableSkeletonRows };
