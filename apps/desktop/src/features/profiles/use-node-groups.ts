import { useMemo, useRef, useState } from "react";

import { useNodeListStore } from "@voya/client/node-list-store";

/** Collapsed groups and the list's order and filter, kept across launches. */
export function useNodeGroups() {
  const [search, setSearch] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const collapsedGroups = useNodeListStore((state) => state.collapsedGroups);
  const hideUnreachable = useNodeListStore((state) => state.hideUnreachable);
  const sortByLatency = useNodeListStore((state) => state.sortByLatency);
  const collapsed = useMemo(() => new Set(collapsedGroups), [collapsedGroups]);
  const { setHideUnreachable, setSortByLatency, toggleGroup } = useNodeListStore.getState();
  return {
    search,
    setSearch,
    searchRef,
    clearSearch: () => { setSearch(""); searchRef.current?.focus(); },
    collapsed,
    hideUnreachable,
    setHideUnreachable,
    setSortByLatency,
    sortByLatency,
    toggle: toggleGroup,
  };
}
