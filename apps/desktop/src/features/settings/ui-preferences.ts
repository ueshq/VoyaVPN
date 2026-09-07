import { useQuery } from "@tanstack/react-query";

import {
  applyDocumentLocale,
  changeLocale,
  getInitialLocale,
  i18next,
  localeOptions,
  type Locale,
} from "@voya/i18n";
import { loadUiPreferences } from "@/ipc";
import { queryKeys } from "@/ipc/query-keys";
import type { AppearanceSettings } from "@/ipc/bindings";
import {
  isThemeMode,
  type ThemeMode,
  usePreferencesStore,
} from "@/stores/preferences-store";

type NormalizedUiPreferences = AppearanceSettings & {
  language: Locale;
  theme: ThemeMode;
};

export function useUiPreferencesQuery() {
  return useQuery({
    queryFn: loadUiPreferences,
    queryKey: queryKeys.uiPreferences,
    select: normalizeUiPreferences,
  });
}

function normalizeUiPreferences(preferences: AppearanceSettings): NormalizedUiPreferences {
  return {
    language: isLocale(preferences.language) ? preferences.language : getInitialLocale(),
    theme: isThemeMode(preferences.theme) ? preferences.theme : "system",
  };
}

/**
 * Apply an appearance bundle to the running UI.
 *
 * `persist: false` is the Settings preview mode: the theme lands in the store's
 * transient `themePreview` slot and the locale switches without touching the
 * stored preference, so an unsaved edit can never become the persisted one and
 * a discard needs no localStorage rollback.
 */
export async function applyUiPreferences(
  preferences: AppearanceSettings,
  options?: { persist?: boolean },
) {
  const normalized = normalizeUiPreferences(preferences);
  const persist = options?.persist !== false;
  const preferencesStore = usePreferencesStore.getState();
  if (persist) {
    preferencesStore.setThemeMode(normalized.theme);
  } else {
    preferencesStore.setThemePreview(normalized.theme);
  }

  const currentLanguage = i18next.resolvedLanguage ?? i18next.language;
  if (currentLanguage === normalized.language) {
    applyDocumentLocale(normalized.language);
    return;
  }

  await changeLocale(normalized.language, { persist });
}

function isLocale(value: string): value is Locale {
  return localeOptions.some((locale) => locale.code === value);
}
