import type * as React from "react";
import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/utils";

// Shared page-shell primitives. Every feature screen used to hand-write the same
// `flex h-full min-h-0 flex-col` section wrapped around a 56px header bar, and the
// spacing scale (gap / padding / min height) drifted between screens. Centralising
// the geometry here makes that scale canonical: the header is `min-h-14` tall with
// `px-4 py-2` padding and `gap-2` between toolbar items, and the title cluster is a
// `gap-2` row of icon + heading + badges. Screens stay free to compose whatever
// toolbar controls they need as children.

function PageSection({ className, ...props }: React.ComponentProps<"section">) {
  return (
    <section className={cn("flex h-full min-h-0 flex-col", className)} data-slot="page-section" {...props} />
  );
}

function PageHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn("flex min-h-14 shrink-0 flex-wrap items-center gap-2 border-b px-4 py-2", className)}
      data-slot="page-header"
      {...props}
    />
  );
}

function PageHeaderHeading({
  children,
  className,
  icon: Icon,
  title,
  ...props
}: React.ComponentProps<"div"> & {
  icon?: LucideIcon;
  title: React.ReactNode;
}) {
  return (
    <div className={cn("flex min-w-0 items-center gap-2", className)} data-slot="page-header-heading" {...props}>
      {Icon ? <Icon className="size-4 text-muted-foreground" aria-hidden="true" /> : null}
      <h2 className="text-sm font-semibold">{title}</h2>
      {children}
    </div>
  );
}

export { PageHeader, PageHeaderHeading, PageSection };
