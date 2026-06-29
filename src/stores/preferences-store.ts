import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";

import {
  DEFAULT_FONT,
  type Font,
  fontFromFamilyString,
  fontToClassName,
  fontToCss,
  fontToFamilyString,
  isFont,
} from "@/config/fonts";
import type { UiItem_Serialize } from "@/ipc/bindings";

export type ThemeMode = "system" | "light" | "dark";
export const DEFAULT_FONT_SIZE = 14;
export const FONT_SIZE_MAX = 20;
export const FONT_SIZE_MIN = 8;

type PersistedPreferences = {
  font: Font;
  fontSize: number;
  themeMode: ThemeMode;
};

type PreferencesState = {
  appConfigLoaded: boolean;
  font: Font;
  fontSize: number;
  hydrateFromConfig: (uiItem: UiItem_Serialize | null | undefined) => void;
  setFont: (font: Font) => void;
  setFontSize: (fontSize: number) => void;
  setThemeMode: (themeMode: ThemeMode) => void;
  themeMode: ThemeMode;
};

export const usePreferencesStore = create<PreferencesState>()(
  persist(
    (set) => ({
      appConfigLoaded: false,
      font: DEFAULT_FONT,
      fontSize: DEFAULT_FONT_SIZE,
      hydrateFromConfig: (uiItem) =>
        set({
          ...preferencesFromConfig(uiItem),
          appConfigLoaded: true,
        }),
      setFont: (font) => set({ font }),
      setFontSize: (fontSize) => set({ fontSize: normalizeFontSize(fontSize) }),
      setThemeMode: (themeMode) => set({ themeMode }),
      themeMode: "system",
    }),
    {
      name: "voyavpn.preferences",
      partialize: (state): PersistedPreferences => ({
        font: state.font,
        fontSize: state.fontSize,
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

function preferencesFromConfig(
  uiItem: UiItem_Serialize | null | undefined,
): PersistedPreferences {
  return {
    font: fontFromFamilyString(uiItem?.CurrentFontFamily),
    fontSize: normalizeFontSize(uiItem?.CurrentFontSize),
    themeMode: themeModeFromConfig(uiItem?.CurrentTheme),
  };
}

export function uiItemWithoutLegacyColor(
  uiItem: UiItem_Serialize | null | undefined,
): Partial<UiItem_Serialize> {
  const nextUiItem: Partial<UiItem_Serialize> = { ...(uiItem ?? {}) };
  delete nextUiItem.ColorPrimaryName;
  return nextUiItem;
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

export function normalizeFontSize(value: number | null | undefined) {
  if (!Number.isFinite(value) || !value) {
    return DEFAULT_FONT_SIZE;
  }

  return Math.min(FONT_SIZE_MAX, Math.max(FONT_SIZE_MIN, Math.round(value)));
}

function mergePersistedPreferences(persistedState: unknown, currentState: PreferencesState): PreferencesState {
  if (!isRecord(persistedState)) {
    return currentState;
  }

  const legacyFamily = typeof persistedState.fontFamily === "string" ? persistedState.fontFamily : undefined;
  const persistedFont = persistedState.font;

  return {
    ...currentState,
    font: isFont(persistedFont) ? persistedFont : fontFromFamilyString(legacyFamily),
    fontSize: normalizeFontSize(typeof persistedState.fontSize === "number" ? persistedState.fontSize : undefined),
    themeMode: typeof persistedState.themeMode === "string" ? themeModeFromConfig(persistedState.themeMode) : "system",
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

export {
  DEFAULT_FONT,
  fontFromFamilyString,
  fontToClassName,
  fontToCss,
  fontToFamilyString,
};
export type { Font };
