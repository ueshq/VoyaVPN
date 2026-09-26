import { useEffect } from "react";
import { getErrorMessage } from "@voya/utils/error";

import { applyUiPreferences, useUiPreferencesQuery } from "@voya/features/settings/ui-preferences";
import type { ThemeMode } from "@voya/contracts";
import { usePreferencesStore } from "@voya/client/preferences-store";

export function PreferencesBridge() {
  const preferencesQuery = useUiPreferencesQuery();
  // A Settings preview lasts until acknowledgement or leaving the page.
  const themeMode = usePreferencesStore((state) => state.themePreview ?? state.themeMode);

  useEffect(() => {
    if (preferencesQuery.data) {
      // The theme is applied before anything can fail; a failed locale load
      // keeps the current language, so there is nothing to show the user, but
      // the failure must not vanish without a trace.
      void applyUiPreferences(preferencesQuery.data).catch((error: unknown) => {
        console.error(`[preferences-bridge] apply: ${getErrorMessage(error)}`);
      });
    }
  }, [preferencesQuery.data]);

  // `changeLocale` already stamps `lang`/`dir` on the document element, so the
  // bridge does not repeat it; it only owns the theme class.
  useThemeEffects(themeMode);

  return null;
}

function useThemeEffects(themeMode: ThemeMode) {
  useEffect(() => {
    const root = document.documentElement;
    // Optional only for jsdom, which has no `matchMedia`.
    const media = window.matchMedia?.("(prefers-color-scheme: dark)");

    const applyTheme = () => {
      const resolvedTheme =
        themeMode === "system" ? (media?.matches ? "dark" : "light") : themeMode;

      root.classList.toggle("dark", resolvedTheme === "dark");
      root.style.colorScheme = resolvedTheme;
    };

    applyTheme();

    if (themeMode !== "system" || !media) {
      return undefined;
    }

    media.addEventListener("change", applyTheme);

    return () => media.removeEventListener("change", applyTheme);
  }, [themeMode]);
}
