import type * as React from "react";
import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/utils";

function EmptyState({
  className,
  description,
  icon: Icon,
  title,
  ...props
}: React.ComponentProps<"div"> & {
  description?: React.ReactNode;
  icon?: LucideIcon;
  title: React.ReactNode;
}) {
  return (
    <div
      className={cn("grid place-items-center gap-2 px-4 py-8 text-center", className)}
      data-slot="empty-state"
      role="status"
      {...props}
    >
      {Icon ? <Icon className="size-8 text-muted-foreground" aria-hidden="true" /> : null}
      <p className="text-sm font-medium text-foreground">{title}</p>
      {description ? <p className="max-w-sm text-sm text-muted-foreground">{description}</p> : null}
    </div>
  );
}

export { EmptyState };
