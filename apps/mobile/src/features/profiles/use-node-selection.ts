import { useNodeListStore } from "@voya/client/node-list-store";
import type { NodeListSelection } from "@voya/features/profiles/use-node-list-data";
import { useMemo, useState } from "react";

/**
 * What the node list is showing, and the handles to change it.
 *
 * The persisted half — collapsed groups, the order and the filter — is the
 * shared `node-list-store`, so a phone and the desktop agree on what the view
 * means. The search box is per visit on both.
 *
 * The desktop's equivalent is `use-node-groups.ts`; it stays separate because
 * it also owns a DOM input ref for its clear button.
 */
export function useNodeSelection(): NodeListSelection & {
  clearSearch: () => void;
  setSearch: (search: string) => void;
  toggleGroup: (groupKey: string) => void;
} {
  const [search, setSearch] = useState("");
  const collapsedGroups = useNodeListStore((state) => state.collapsedGroups);
  const hideUnreachable = useNodeListStore((state) => state.hideUnreachable);
  const sortByLatency = useNodeListStore((state) => state.sortByLatency);
  const collapsed = useMemo(() => new Set(collapsedGroups), [collapsedGroups]);

  return {
    clearSearch: () => setSearch(""),
    collapsed,
    hideUnreachable,
    search,
    setSearch,
    sortByLatency,
    toggleGroup: useNodeListStore.getState().toggleGroup,
  };
}
