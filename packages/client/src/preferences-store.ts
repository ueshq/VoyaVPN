import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

import type { ThemeMode } from "@voya/contracts";

import { mergeValidated } from "./persisted";
import { clientStorage } from "./platform";

/** Every mode the backend knows; a new one fails the typecheck here. */
const THEME_MODES = { dark: true, light: true, system: true } satisfies Record<ThemeMode, true>;

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
      storage: createJSONStorage(() => clientStorage()),
    },
  ),
);

export function isThemeMode(value: unknown): value is ThemeMode {
  return typeof value === "string" && Object.hasOwn(THEME_MODES, value);
}
