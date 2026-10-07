import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import { mergeValidated } from "./persisted";
import { clientStorage } from "./platform";

type ConnectionPreferences = {
  autoConnect: boolean;
  recentIds: string[];
  setAutoConnect: (enabled: boolean) => void;
  rememberConnected: (id: string) => void;
};

/** Device-only choices; recent entries are recorded only after a confirmed connection. */
export const useConnectionPreferences = create<ConnectionPreferences>()(
  persist(
    (set) => ({
      autoConnect: false,
      recentIds: [],
      setAutoConnect: (autoConnect) => set({ autoConnect }),
      rememberConnected: (id) =>
        set((state) => ({ recentIds: [id, ...state.recentIds.filter((item) => item !== id)].slice(0, 5) })),
    }),
    {
      name: "voyavpn.mobile.connection",
      storage: createJSONStorage(() => clientStorage()),
      partialize: ({ autoConnect, recentIds }) => ({ autoConnect, recentIds }),
      merge: mergeValidated<ConnectionPreferences>(({ autoConnect, recentIds }) => ({
        autoConnect: autoConnect === true,
        recentIds: Array.isArray(recentIds)
          ? [...new Set(recentIds.filter((id): id is string => typeof id === "string" && id.length > 0))].slice(0, 5)
          : [],
      })),
    },
  ),
);
