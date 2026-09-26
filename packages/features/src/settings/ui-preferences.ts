import { queries } from "@voya/client/queries";
import { useQuery } from "@tanstack/react-query";

import { i18next, i18nHost, isLocale, type Locale } from "@voya/i18n/core";
import { toastError, useToastStore } from "@voya/client/toast-store";
import type { AppearanceSettings, ThemeMode } from "@voya/contracts";
import { isThemeMode, usePreferencesStore } from "@voya/client/preferences-store";

type NormalizedUiPreferences = AppearanceSettings & {
  language: Locale;
  theme: ThemeMode;
};

let preview: { owner: symbol; preferences: AppearanceSettings } | null = null;

export function previewUiPreferences(owner: symbol, preferences: AppearanceSettings) {
  preview = { owner, preferences };
  void applyUiPreferences(preferences, { persist: false }).catch(reportUiPreferencesError);
}

export function reportUiPreferencesError(error: unknown) {
  toastError(i18next.t("status.operationFailed"), error);
}

/** Ends the owner's preview and returns what it was showing, if anything. */
export function endUiPreferencesPreview(owner: symbol): AppearanceSettings | null {
  if (preview?.owner !== owner) return null;
  const ended = preview.preferences;
  preview = null;
  usePreferencesStore.getState().setThemePreview(null);
  return ended;
}

/** A previewed theme or language that never saved is being switched back. */
export function reportUiPreferencesReverted() {
  useToastStore.getState().pushToast({
    title: i18next.t("settings.appearanceReverted.title"),
    description: i18next.t("settings.appearanceReverted.description"),
    severity: "error",
  });
}

export function useUiPreferencesQuery() {
  return useQuery({
    ...queries.uiPreferences,
    select: normalizeUiPreferences,
  });
}

function normalizeUiPreferences(preferences: AppearanceSettings): NormalizedUiPreferences {
  return {
    language: isLocale(preferences.language) ? preferences.language : i18nHost().getInitialLocale(),
    theme: isThemeMode(preferences.theme) ? preferences.theme : "system",
  };
}

/**
 * Apply an appearance bundle to the running UI.
 *
 * `persist: false` is the Settings preview mode: the theme lands in the store's
 * transient `themePreview` slot and the locale switches without touching the
 * stored preference. Only the backend acknowledgement persists it; leaving
 * clears any failed preview.
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
    i18nHost().applyLocale(normalized.language);
  } else {
    await i18nHost().changeLocale(normalized.language, { persist });
  }
  // A cache refresh or an older save must not replace the user's newer preview.
  if (persist && preview) await applyUiPreferences(preview.preferences, { persist: false });
}
