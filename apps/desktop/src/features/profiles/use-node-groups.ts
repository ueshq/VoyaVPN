import { useMemo } from "react";

import { useNodeListStore } from "@/stores/node-list-store";

/** Collapsed groups and the list's order and filter, kept across launches. */
export function useNodeGroups() {
  const collapsedGroups = useNodeListStore((state) => state.collapsedGroups);
  const hideUnreachable = useNodeListStore((state) => state.hideUnreachable);
  const sortByLatency = useNodeListStore((state) => state.sortByLatency);
  const collapsed = useMemo(() => new Set(collapsedGroups), [collapsedGroups]);
  const { setHideUnreachable, setSortByLatency, toggleGroup } = useNodeListStore.getState();
  return {
    collapsed,
    hideUnreachable,
    setHideUnreachable,
    setSortByLatency,
    sortByLatency,
    toggle: toggleGroup,
  };
}
