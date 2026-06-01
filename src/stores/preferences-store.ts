import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

import type { UiItem_Serialize } from "@/ipc/bindings";

export type ThemeMode = "system" | "light" | "dark";
export type Accent = "teal" | "blue" | "rose";

export const DEFAULT_FONT_FAMILY =
  'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
export const DEFAULT_FONT_SIZE = 16;
export const FONT_SIZE_MAX = 20;
export const FONT_SIZE_MIN = 8;

type PersistedPreferences = {
  accent: Accent;
  fontFamily: string;
  fontSize: number;
  themeMode: ThemeMode;
};

type PreferencesState = {
  accent: Accent;
  appConfigLoaded: boolean;
  fontFamily: string;
  fontSize: number;
  hydrateFromConfig: (uiItem: UiItem_Serialize | null | undefined) => void;
  setAccent: (accent: Accent) => void;
  setFontFamily: (fontFamily: string) => void;
  setFontSize: (fontSize: number) => void;
  setThemeMode: (themeMode: ThemeMode) => void;
  themeMode: ThemeMode;
};

export const usePreferencesStore = create<PreferencesState>()(
  persist(
    (set) => ({
      accent: "teal",
      appConfigLoaded: false,
      fontFamily: "",
      fontSize: DEFAULT_FONT_SIZE,
      hydrateFromConfig: (uiItem) =>
        set({
          ...preferencesFromConfig(uiItem),
          appConfigLoaded: true,
        }),
      setAccent: (accent) => set({ accent }),
      setFontFamily: (fontFamily) => set({ fontFamily }),
      setFontSize: (fontSize) => set({ fontSize: normalizeFontSize(fontSize) }),
      setThemeMode: (themeMode) => set({ themeMode }),
      themeMode: "system",
    }),
    {
      name: "voyavpn.preferences",
      partialize: (state): PersistedPreferences => ({
        accent: state.accent,
        fontFamily: state.fontFamily,
        fontSize: state.fontSize,
        themeMode: state.themeMode,
      }),
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

export function preferencesFromConfig(
  uiItem: UiItem_Serialize | null | undefined,
): PersistedPreferences {
  return {
    accent: accentFromConfig(uiItem?.ColorPrimaryName),
    fontFamily: uiItem?.CurrentFontFamily?.trim() ?? "",
    fontSize: normalizeFontSize(uiItem?.CurrentFontSize),
    themeMode: themeModeFromConfig(uiItem?.CurrentTheme),
  };
}

export function themeModeFromConfig(value: string | null | undefined): ThemeMode {
  switch ((value ?? "").trim().toLowerCase()) {
    case "dark":
      return "dark";
    case "light":
      return "light";
    case "followsystem":
    case "follow-system":
    case "system":
    default:
      return "system";
  }
}

export function themeModeToConfig(themeMode: ThemeMode) {
  switch (themeMode) {
    case "dark":
      return "Dark";
    case "light":
      return "Light";
    case "system":
      return "FollowSystem";
  }
}

export function accentFromConfig(value: string | null | undefined): Accent {
  switch ((value ?? "").trim().toLowerCase()) {
    case "blue":
    case "sky":
      return "blue";
    case "rose":
    case "red":
      return "rose";
    case "teal":
    default:
      return "teal";
  }
}

export function accentToConfig(accent: Accent) {
  switch (accent) {
    case "blue":
      return "Blue";
    case "rose":
      return "Rose";
    case "teal":
      return "Teal";
  }
}

export function normalizeFontSize(value: number | null | undefined) {
  if (!Number.isFinite(value) || !value) {
    return DEFAULT_FONT_SIZE;
  }

  return Math.min(FONT_SIZE_MAX, Math.max(FONT_SIZE_MIN, Math.round(value)));
}

export function fontFamilyToCss(fontFamily: string) {
  return fontFamily.trim() || DEFAULT_FONT_FAMILY;
}
