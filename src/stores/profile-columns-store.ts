import { functionalUpdate, type Updater, type VisibilityState } from "@tanstack/react-table";
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

/**
 * Default profiles-table column visibility. Only the high-signal columns ship
 * visible — protocol / remarks / address / delay / group — while niche columns
 * (port, transport, security, speed, per-server traffic, IP info) start
 * collapsed behind the "Columns" menu to cut the forced horizontal scroll. The
 * structural `state` (#) column is intentionally omitted here so it stays
 * permanently visible (TanStack treats a missing id as visible).
 */
export const DEFAULT_PROFILE_COLUMN_VISIBILITY: VisibilityState = {
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
  subid: true,
};

type ProfileColumnsState = {
  columnVisibility: VisibilityState;
  resetColumnVisibility: () => void;
  setColumnVisibility: (updater: Updater<VisibilityState>) => void;
};

export const useProfileColumnsStore = create<ProfileColumnsState>()(
  persist(
    (set) => ({
      columnVisibility: { ...DEFAULT_PROFILE_COLUMN_VISIBILITY },
      resetColumnVisibility: () => set({ columnVisibility: { ...DEFAULT_PROFILE_COLUMN_VISIBILITY } }),
      setColumnVisibility: (updater) =>
        set((state) => ({ columnVisibility: functionalUpdate(updater, state.columnVisibility) })),
    }),
    {
      name: "voyavpn.profileColumns",
      partialize: (state) => ({ columnVisibility: state.columnVisibility }),
      // Overlay persisted choices on top of the current defaults so columns
      // added in a future release inherit their default visibility instead of
      // disappearing for users with an older persisted map.
      merge: (persistedState, currentState) => ({
        ...currentState,
        columnVisibility: {
          ...DEFAULT_PROFILE_COLUMN_VISIBILITY,
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
