import type * as React from "react";

import { Badge } from "@voya/ui/components/badge";
import { cn } from "@voya/ui/lib/utils";

// Shared page-shell primitives. Every feature screen used to hand-write the same
// `flex h-full min-h-0 flex-col` section wrapped around a 56px header bar, and the
// spacing scale (gap / padding / min height) drifted between screens. Centralising
// the geometry here makes that scale canonical: the header is `min-h-14` tall with
// `px-4 py-2` padding and `gap-2` between toolbar items on a raised surface, and the
// title cluster is a `gap-2` row of icon + heading + optional count badge. Screens
// compose toolbar controls as children, parked to the trailing edge via
// `PageHeaderActions` so the `ms-auto` push is canonical rather than hand-rolled.

function PageSection({ className, ...props }: React.ComponentProps<"section">) {
  return (
    <section className={cn("flex h-full min-h-0 flex-col", className)} data-slot="page-section" {...props} />
  );
}

// Hiddify-style large page identity: the page's single `<h1>` above the toolbar
// strip. Screens keep `PageHeader` as a pure toolbar row; embedded/secondary
// surfaces render their own small `<h2>` underneath this h1.
function PageTitle({
  actions,
  className,
  count,
  title,
  ...props
}: React.ComponentProps<"div"> & {
  actions?: React.ReactNode;
  count?: React.ReactNode;
  title: React.ReactNode;
}) {
  return (
    <div
      className={cn("flex shrink-0 items-center gap-3 px-6 pt-5 pb-3", className)}
      data-slot="page-title"
      {...props}
    >
      <h1 className="min-w-0 truncate text-2xl font-semibold tracking-tight">{title}</h1>
      {count == null ? null : (
        <Badge className="h-6 bg-background tabular-nums text-muted-foreground" variant="outline">
          {count}
        </Badge>
      )}
      {actions ? <div className="ms-auto flex items-center gap-2">{actions}</div> : null}
    </div>
  );
}

function PageHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "flex min-h-14 shrink-0 flex-wrap items-center gap-2 border-b bg-surface-raised px-4 py-2",
        className,
      )}
      data-slot="page-header"
      {...props}
    />
  );
}

// Trailing toolbar cluster: parks controls against the header's end edge with the
// canonical `ms-auto` push so screens stop re-deriving it inline.
function PageHeaderActions({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn("ms-auto flex items-center gap-2", className)}
      data-slot="page-header-actions"
      {...props}
    />
  );
}

export { PageHeader, PageHeaderActions, PageSection, PageTitle };
