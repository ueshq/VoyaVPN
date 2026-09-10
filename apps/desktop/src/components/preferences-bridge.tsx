import { useEffect } from "react";

import { applyUiPreferences, useUiPreferencesQuery } from "@/features/settings/ui-preferences";
import {
  resolveThemeMode,
  type ThemeMode,
  usePreferencesStore,
} from "@/stores/preferences-store";

export function PreferencesBridge() {
  const preferencesQuery = useUiPreferencesQuery();
  // A Settings preview lasts until acknowledgement or leaving the page.
  const themeMode = usePreferencesStore((state) => state.themePreview ?? state.themeMode);

  useEffect(() => {
    if (preferencesQuery.data) {
      void applyUiPreferences(preferencesQuery.data).catch(() => undefined);
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
    const media =
      typeof window.matchMedia === "function"
        ? window.matchMedia("(prefers-color-scheme: dark)")
        : undefined;

    const applyTheme = () => {
      const resolvedTheme = resolveThemeMode(themeMode);

      root.classList.toggle("dark", resolvedTheme === "dark");
      root.style.colorScheme = resolvedTheme;
    };

    applyTheme();

    if (
      themeMode !== "system" ||
      !media ||
      typeof media.addEventListener !== "function" ||
      typeof media.removeEventListener !== "function"
    ) {
      return undefined;
    }

    media.addEventListener("change", applyTheme);

    return () => media.removeEventListener("change", applyTheme);
  }, [themeMode]);
}
