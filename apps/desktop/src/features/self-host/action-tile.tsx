import type { ReactNode } from "react";
import { ChevronRight, type LucideIcon } from "lucide-react";

import { cn } from "@voya/ui/lib/utils";

import { pageSurfaceClassName } from "@/components/app-shell/page-section";

/**
 * A way into one of the page's dialogs that already says where things stand,
 * so a glance at the page answers most questions without opening anything.
 */
export function ActionTile({
  icon: Icon,
  onClick,
  summary,
  title,
}: {
  icon: LucideIcon;
  onClick: () => void;
  summary: ReactNode;
  title: string;
}) {
  return (
    <button
      className={cn(
        pageSurfaceClassName,
        "group/tile grid min-w-0 gap-2 p-4 text-start outline-none transition-colors",
        // The hover tint is a layer, so the raised surface underneath stays opaque.
        "hover:bg-[image:linear-gradient(var(--surface-hovered),var(--surface-hovered))]",
        "active:bg-[image:linear-gradient(var(--surface-pressed),var(--surface-pressed))]",
        "focus-visible:ring-2 focus-visible:ring-ring",
      )}
      onClick={onClick}
      type="button"
    >
      <span className="flex min-w-0 items-center gap-2">
        <Icon aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
        <span className="min-w-0 truncate text-sm font-semibold">{title}</span>
        <ChevronRight
          aria-hidden="true"
          className="ms-auto size-4 shrink-0 text-muted-foreground transition-transform duration-short group-hover/tile:translate-x-0.5 rtl:rotate-180"
        />
      </span>
      <span className="min-w-0 truncate text-xs text-muted-foreground">{summary}</span>
    </button>
  );
}
