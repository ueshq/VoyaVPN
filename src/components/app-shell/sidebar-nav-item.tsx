import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/utils";

// A single sidebar destination row. Rendered as an ARIA `tab` (rather than a
// plain button or link) so the shell's nav reads as a tablist and the existing
// `getByRole("tab")` assertions keep holding after the Radix Tabs drop. Selected
// rows read blue (`accent-blue-light` tint + `text-primary`); the rest fall back
// to a calm surface hover.
export function SidebarNavItem({
  active,
  icon: Icon,
  id,
  label,
  onSelect,
  panelId,
}: {
  active: boolean;
  icon: LucideIcon;
  id: string;
  label: string;
  onSelect: () => void;
  panelId: string;
}) {
  return (
    <button
      aria-controls={panelId}
      aria-selected={active}
      className={cn(
        "flex w-full items-center gap-2 rounded-sm py-1.5 pr-4 pl-3 text-sm font-medium outline-none transition-colors duration-short ease-out-practical focus-visible:ring-2 focus-visible:ring-ring/50",
        active
          ? "bg-accent-blue-light text-primary"
          : "text-sidebar-foreground hover:bg-accent",
      )}
      id={id}
      onClick={onSelect}
      role="tab"
      tabIndex={active ? 0 : -1}
      type="button"
    >
      <Icon className="size-4 shrink-0" aria-hidden="true" />
      <span className="flex-1 truncate text-start">{label}</span>
    </button>
  );
}
