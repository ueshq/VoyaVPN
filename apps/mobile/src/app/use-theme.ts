import { usePreferencesStore } from "@voya/client/preferences-store";
import { useEffect, useSyncExternalStore } from "react";
import { Appearance } from "react-native";
import { Uniwind } from "uniwind";

function subscribeToAppearance(onChange: () => void) {
  const subscription = Appearance.addChangeListener(onChange);

  return () => {
    subscription.remove();
  };
}

function appearanceSnapshot() {
  return Appearance.getColorScheme() === "dark" ? "dark" : "light";
}

/**
 * The resolved light/dark theme.
 *
 * The preference itself is the shared one: `themeMode` (`light`/`dark`/
 * `system`) lives in `@voya/client` and persists to MMKV, so both shells agree
 * on what the setting means. Only the resolution of `system` is
 * platform-specific.
 *
 * `Appearance` is subscribed to rather than read in an effect, so a running app
 * follows the OS switch without a second render pass to correct itself.
 *
 * Uniwind keeps its own current theme — it is what every `dark:` class resolves
 * against — so the preference is pushed into it as well. Its vocabulary is the
 * same three words, including `system`, so nothing is translated on the way.
 */
export function useTheme() {
  const themeMode = usePreferencesStore((state) => state.themePreview ?? state.themeMode);
  const systemScheme = useSyncExternalStore(subscribeToAppearance, appearanceSnapshot);

  const resolved = themeMode === "system" ? systemScheme : themeMode;

  // The resolved scheme, never the word "system". Uniwind understands
  // "system", but on React Native 0.87 its adaptive path writes `"auto"` back
  // through `Appearance` and then reads that same `"auto"` out of its own
  // change listener as if it were a theme name. Nothing matches it, so every
  // theme variable resolves to nothing: layout classes still work and every
  // colour silently disappears.
  useEffect(() => {
    Uniwind.setTheme(resolved);

  }, [resolved]);

  return resolved;
}
