import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

import { mergeValidated } from "./persisted";

export type ThemeMode = "system" | "light" | "dark";

type PersistedPreferences = {
  /** When the rule library was last updated from this device; the backend keeps no date. */
  ruleLibraryUpdatedAt: number | null;
  themeMode: ThemeMode;
};

type PreferencesState = {
  ruleLibraryUpdatedAt: number | null;
  setRuleLibraryUpdatedAt: (time: number) => void;
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
      ruleLibraryUpdatedAt: null,
      setRuleLibraryUpdatedAt: (ruleLibraryUpdatedAt) => set({ ruleLibraryUpdatedAt }),
      setThemeMode: (themeMode) => set({ themeMode, themePreview: null }),
      setThemePreview: (themePreview) => set({ themePreview }),
      themeMode: "system",
      themePreview: null,
    }),
    {
      name: "voyavpn.preferences",
      partialize: (state): PersistedPreferences => ({
        ruleLibraryUpdatedAt: state.ruleLibraryUpdatedAt,
        themeMode: state.themeMode,
      }),
      merge: mergeValidated<PreferencesState>(({ ruleLibraryUpdatedAt, themeMode }) => ({
        ...(typeof ruleLibraryUpdatedAt === "number" && Number.isFinite(ruleLibraryUpdatedAt)
          ? { ruleLibraryUpdatedAt }
          : {}),
        ...(isThemeMode(themeMode) ? { themeMode } : {}),
      })),
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
