import { usePreferencesStore } from "@voya/client/preferences-store";
import { useSyncExternalStore } from "react";
import { Appearance } from "react-native";

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
 */
export function useTheme() {
  const themeMode = usePreferencesStore((state) => state.themePreview ?? state.themeMode);
  const systemScheme = useSyncExternalStore(subscribeToAppearance, appearanceSnapshot);

  return themeMode === "system" ? systemScheme : themeMode;
}
