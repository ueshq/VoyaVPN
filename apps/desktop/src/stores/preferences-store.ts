import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

export type ThemeMode = "system" | "light" | "dark";

type PersistedPreferences = {
  themeMode: ThemeMode;
};

type PreferencesState = {
  setThemeMode: (themeMode: ThemeMode) => void;
  setThemePreview: (themePreview: ThemeMode | null) => void;
  themeMode: ThemeMode;
  /**
   * Transient override applied while the Settings surface previews an unsaved
   * appearance. It is deliberately outside `partialize`, so a preview never
   * reaches localStorage and a discarded preview needs no rollback.
   */
  themePreview: ThemeMode | null;
};

export const usePreferencesStore = create<PreferencesState>()(
  persist(
    (set) => ({
      // Committing a theme also ends any preview of it.
      setThemeMode: (themeMode) => set({ themeMode, themePreview: null }),
      setThemePreview: (themePreview) => set({ themePreview }),
      themeMode: "system",
      themePreview: null,
    }),
    {
      name: "voyavpn.preferences",
      partialize: (state): PersistedPreferences => ({
        themeMode: state.themeMode,
      }),
      merge: (persistedState, currentState) => mergePersistedPreferences(persistedState, currentState),
      storage: createJSONStorage(() => window.localStorage),
    },
  ),
);

export function resolveThemeMode(themeMode: ThemeMode) {
  if (themeMode !== "system") {
    return themeMode;
  }

  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return "light";
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function isThemeMode(value: unknown): value is ThemeMode {
  return value === "system" || value === "light" || value === "dark";
}

function mergePersistedPreferences(persistedState: unknown, currentState: PreferencesState): PreferencesState {
  if (!isRecord(persistedState)) {
    return currentState;
  }

  return {
    ...currentState,
    themeMode: isThemeMode(persistedState.themeMode) ? persistedState.themeMode : "system",
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
