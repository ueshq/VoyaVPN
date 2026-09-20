import { useEffect } from "react";
import { getErrorMessage } from "@voya/utils/error";

import { applyUiPreferences, useUiPreferencesQuery } from "@/features/settings/ui-preferences";
import type { ThemeMode } from "@/ipc/bindings";
import { resolveThemeMode, usePreferencesStore } from "@voya/client/preferences-store";

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
