import { functionalUpdate, type Updater, type VisibilityState } from "@tanstack/react-table";
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

type ColumnVisibilityState = {
  columnVisibility: VisibilityState;
  resetColumnVisibility: () => void;
  setColumnVisibility: (updater: Updater<VisibilityState>) => void;
};

/**
 * Builds a persisted column-visibility store for one data table.
 *
 * The persisted-state migration semantics — overlay the stored choices on the
 * current defaults, and ignore anything that is not a boolean — have to stay
 * identical across tables, so they live here once instead of being copied per
 * table. Only the storage key and the default map differ.
 */
export function createColumnVisibilityStore(storageKey: string, defaults: VisibilityState) {
  return create<ColumnVisibilityState>()(
    persist(
      (set) => ({
        columnVisibility: { ...defaults },
        resetColumnVisibility: () => set({ columnVisibility: { ...defaults } }),
        setColumnVisibility: (updater) =>
          set((state) => ({ columnVisibility: functionalUpdate(updater, state.columnVisibility) })),
      }),
      {
        name: storageKey,
        partialize: (state) => ({ columnVisibility: state.columnVisibility }),
        // Overlay persisted choices on top of the current defaults so columns
        // added in a future release inherit their default visibility instead of
        // disappearing for users with an older persisted map.
        merge: (persistedState, currentState) => ({
          ...currentState,
          columnVisibility: { ...defaults, ...readPersistedVisibility(persistedState) },
        }),
        storage: createJSONStorage(() => window.localStorage),
      },
    ),
  );
}

function readPersistedVisibility(persistedState: unknown): VisibilityState {
  if (!persistedState || typeof persistedState !== "object") {
    return {};
  }

  const candidate = (persistedState as { columnVisibility?: unknown }).columnVisibility;

  if (!candidate || typeof candidate !== "object") {
    return {};
  }

  const result: VisibilityState = {};

  for (const [key, value] of Object.entries(candidate as Record<string, unknown>)) {
    if (typeof value === "boolean") {
      result[key] = value;
    }
  }

  return result;
}
