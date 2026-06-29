import { ChevronDown, ChevronRight } from "lucide-react";
import type { ReactNode } from "react";

import { useShellStore } from "@/stores/shell-store";

// Collapsible grouping for the sidebar nav. The header row toggles the section's
// `collapsedSections` flag in the shell store; the body (the grouped nav rows) is
// only rendered while expanded. Sections default to expanded — an absent key in
// `collapsedSections` is treated as open — so every destination stays reachable
// (and assertable) without seeding the store.
export function SidebarSectionHeader({
  children,
  id,
  label,
}: {
  children?: ReactNode;
  id: string;
  label: string;
}) {
  const collapsed = useShellStore((state) => Boolean(state.collapsedSections[id]));
  const toggleSection = useShellStore((state) => state.toggleSection);
  const Caret = collapsed ? ChevronRight : ChevronDown;

  return (
    <section className="flex flex-col">
      <button
        aria-expanded={!collapsed}
        className="flex w-full items-center gap-1 rounded-md px-3 py-2 outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
        onClick={() => toggleSection(id)}
        type="button"
      >
        <Caret className="size-3 shrink-0 text-muted-foreground" aria-hidden="true" />
        <span className="flex-1 truncate text-start text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
          {label}
        </span>
      </button>
      {collapsed ? null : children}
    </section>
  );
}
