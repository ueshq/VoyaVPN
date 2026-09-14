import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

import { mergeValidated } from "./persisted";

type NodeListView = {
  collapsedGroups: string[];
  hideUnreachable: boolean;
  sortByLatency: boolean;
};

type NodeListState = NodeListView & {
  setHideUnreachable: (hideUnreachable: boolean) => void;
  setSortByLatency: (sortByLatency: boolean) => void;
  toggleGroup: (groupKey: string) => void;
};

/** How the Nodes page is laid out; remembered between launches. */
export const useNodeListStore = create<NodeListState>()(
  persist(
    (set) => ({
      collapsedGroups: [],
      hideUnreachable: false,
      sortByLatency: false,
      setHideUnreachable: (hideUnreachable) => set({ hideUnreachable }),
      setSortByLatency: (sortByLatency) => set({ sortByLatency }),
      toggleGroup: (groupKey) =>
        set((state) => ({
          collapsedGroups: state.collapsedGroups.includes(groupKey)
            ? state.collapsedGroups.filter((key) => key !== groupKey)
            : [...state.collapsedGroups, groupKey],
        })),
    }),
    {
      name: "voyavpn.nodeList",
      partialize: ({ collapsedGroups, hideUnreachable, sortByLatency }): NodeListView => ({
        collapsedGroups,
        hideUnreachable,
        sortByLatency,
      }),
      merge: mergeValidated<NodeListState>(persistedView),
      storage: createJSONStorage(() => window.localStorage),
    },
  ),
);

function persistedView(record: Record<string, unknown>): Partial<NodeListView> {
  const view: Partial<NodeListView> = {};
  if (Array.isArray(record.collapsedGroups)) {
    view.collapsedGroups = record.collapsedGroups.filter(
      (key): key is string => typeof key === "string",
    );
  }
  if (typeof record.hideUnreachable === "boolean") view.hideUnreachable = record.hideUnreachable;
  if (typeof record.sortByLatency === "boolean") view.sortByLatency = record.sortByLatency;
  return view;
}
