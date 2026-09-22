import { useMemo, useState } from "react";

import { useNodeListStore } from "@voya/client/node-list-store";

/**
 * DOM inputs only — React Native has no equivalent to refocus after clear.
 * Structural so this module stays free of `HTMLInputElement` (mobile tsconfig
 * has no DOM lib).
 */
type SearchFocusRef = { readonly current: { focus?: () => void } | null };

/**
 * What the node list is showing, and the handles to change it.
 *
 * The persisted half — collapsed groups, the order and the filter — is the
 * shared `node-list-store`, so a phone and the desktop agree on what the view
 * means. The search box is per visit on both.
 */
export function useNodeSelection(searchRef?: SearchFocusRef) {
  const [search, setSearch] = useState("");
  const collapsedGroups = useNodeListStore((state) => state.collapsedGroups);
  const hideUnreachable = useNodeListStore((state) => state.hideUnreachable);
  const sortByLatency = useNodeListStore((state) => state.sortByLatency);
  const collapsed = useMemo(() => new Set(collapsedGroups), [collapsedGroups]);
  const { setHideUnreachable, setSortByLatency, toggleGroup } = useNodeListStore.getState();

  return {
    search,
    setSearch,
    clearSearch: () => {
      setSearch("");
      searchRef?.current?.focus?.();
    },
    collapsed,
    hideUnreachable,
    setHideUnreachable,
    setSortByLatency,
    sortByLatency,
    toggleGroup,
  };
}
