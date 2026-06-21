import { functionalUpdate, type Updater, type VisibilityState } from "@tanstack/react-table";
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

/**
 * Default Clash connections-table column visibility. Only the high-signal
 * columns ship visible — host / upload / download / process — while the niche
 * columns (network, source, destination, proxy chain) start collapsed behind
 * the "Columns" menu to cut the forced horizontal scroll. The structural marker
 * track is rendered outside this map so it stays permanently visible.
 */
export const DEFAULT_CONNECTION_COLUMN_VISIBILITY: VisibilityState = {
  host: true,
  network: false,
  source: false,
  destination: false,
  upload: true,
  download: true,
  chain: false,
  process: true,
};

type ConnectionColumnsState = {
  columnVisibility: VisibilityState;
  resetColumnVisibility: () => void;
  setColumnVisibility: (updater: Updater<VisibilityState>) => void;
};

export const useConnectionColumnsStore = create<ConnectionColumnsState>()(
  persist(
    (set) => ({
      columnVisibility: { ...DEFAULT_CONNECTION_COLUMN_VISIBILITY },
      resetColumnVisibility: () => set({ columnVisibility: { ...DEFAULT_CONNECTION_COLUMN_VISIBILITY } }),
      setColumnVisibility: (updater) =>
        set((state) => ({ columnVisibility: functionalUpdate(updater, state.columnVisibility) })),
    }),
    {
      name: "voyavpn.connectionColumns",
      partialize: (state) => ({ columnVisibility: state.columnVisibility }),
      // Overlay persisted choices on top of the current defaults so columns
      // added in a future release inherit their default visibility instead of
      // disappearing for users with an older persisted map.
      merge: (persistedState, currentState) => ({
        ...currentState,
        columnVisibility: {
          ...DEFAULT_CONNECTION_COLUMN_VISIBILITY,
          ...readPersistedVisibility(persistedState),
        },
      }),
      storage: createJSONStorage(() => window.localStorage),
    },
  ),
);

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
