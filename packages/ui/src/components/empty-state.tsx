import type * as React from "react";
import type { LucideIcon } from "lucide-react";

import { cn } from "@voya/ui/lib/utils";

function EmptyState({
  actions,
  className,
  description,
  icon: Icon,
  iconClassName,
  title,
  ...props
}: React.ComponentProps<"div"> & {
  actions?: React.ReactNode;
  description?: React.ReactNode;
  icon?: LucideIcon;
  /** Extra classes for the icon, e.g. `animate-spin` for a waiting state. */
  iconClassName?: string;
  title: React.ReactNode;
}) {
  return (
    <div
      className={cn(
        "grid place-items-center gap-3 px-4 py-12 text-center",
        className,
      )}
      data-slot="empty-state"
      role="status"
      {...props}
    >
      {Icon ? (
        <Icon
          className={cn("size-8 text-muted-foreground", iconClassName)}
          aria-hidden="true"
        />
      ) : null}
      <p className="text-sm font-medium text-foreground">{title}</p>
      {description ? (
        <div className="max-w-sm text-sm text-muted-foreground">
          {description}
        </div>
      ) : null}
      {actions ? (
        <div className="flex flex-wrap justify-center gap-2">{actions}</div>
      ) : null}
    </div>
  );
}

export { EmptyState };
