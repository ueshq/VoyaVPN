import type { LucideIcon } from "lucide-react";

import { cn } from "@voya/ui/lib/utils";

// A single sidebar destination row. Rendered as an ARIA `tab` (rather than a
// plain button or link) so the shell's nav reads as a tablist and the existing
// `getByRole("tab")` assertions keep holding after the Radix Tabs drop. Selected
// rows read blue (`accent-blue-light` tint + `text-brand`); the rest fall back
// to a calm surface hover.
export function SidebarNavItem({
  active,
  collapsed = false,
  icon: Icon,
  id,
  label,
  onSelect,
  panelId,
}: {
  active: boolean;
  collapsed?: boolean;
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
      aria-label={label}
      title={collapsed ? label : undefined}
      className={cn("sidebar-nav-item", active && "sidebar-nav-item-active")}
      id={id}
      onClick={onSelect}
      role="tab"
      tabIndex={active ? 0 : -1}
      type="button"
    >
      <Icon className="size-4 shrink-0" aria-hidden="true" />
      <span className={cn("flex-1 truncate text-start", collapsed && "sr-only")}>{label}</span>
    </button>
  );
}
