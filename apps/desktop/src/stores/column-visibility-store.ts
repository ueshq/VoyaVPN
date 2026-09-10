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
function createColumnVisibilityStore(storageKey: string, defaults: VisibilityState) {
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

/**
 * Default proxy connections-table column visibility. Only the high-signal
 * columns ship visible — host / upload / download / process — while the niche
 * columns (network, source, destination, proxy chain) start collapsed behind
 * the "Columns" menu to cut the forced horizontal scroll. The structural marker
 * track is rendered outside this map so it stays permanently visible.
 */
const DEFAULT_CONNECTION_COLUMN_VISIBILITY: VisibilityState = {
  host: true,
  network: false,
  source: false,
  destination: false,
  upload: true,
  download: true,
  chain: false,
  process: true,
};

export const useConnectionColumnsStore = createColumnVisibilityStore(
  "voyavpn.connectionColumns",
  DEFAULT_CONNECTION_COLUMN_VISIBILITY,
);

/**
 * Default profiles-table column visibility. Only the high-signal columns ship
 * visible — protocol / remarks / address / delay / group — while niche columns
 * (port, transport, security, speed, per-server traffic, IP info) start
 * collapsed behind the "Columns" menu to cut the forced horizontal scroll. The
 * structural `state` (#) column is intentionally omitted here so it stays
 * permanently visible (TanStack treats a missing id as visible).
 */
const DEFAULT_PROFILE_COLUMN_VISIBILITY: VisibilityState = {
  configType: true,
  remarks: true,
  address: true,
  port: false,
  network: false,
  security: false,
  delay: true,
  speed: false,
  todayUp: false,
  todayDown: false,
  totalUp: false,
  totalDown: false,
  ipInfo: false,
  subscriptionId: true,
};

export const useProfileColumnsStore = createColumnVisibilityStore(
  "voyavpn.profileColumns",
  DEFAULT_PROFILE_COLUMN_VISIBILITY,
);
