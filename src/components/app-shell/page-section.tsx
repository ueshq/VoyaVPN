import type * as React from "react";
import type { LucideIcon } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

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

function PageHeaderHeading({
  children,
  className,
  count,
  icon: Icon,
  title,
  ...props
}: React.ComponentProps<"div"> & {
  count?: React.ReactNode;
  icon?: LucideIcon;
  title: React.ReactNode;
}) {
  return (
    <div className={cn("flex min-w-0 items-center gap-2", className)} data-slot="page-header-heading" {...props}>
      {Icon ? <Icon className="size-4 text-muted-foreground" aria-hidden="true" /> : null}
      <h2 className="text-sm font-semibold">{title}</h2>
      {count == null ? null : (
        <Badge className="h-6 bg-background tabular-nums text-muted-foreground" variant="outline">
          {count}
        </Badge>
      )}
      {children}
    </div>
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

export { PageHeader, PageHeaderActions, PageHeaderHeading, PageSection };
