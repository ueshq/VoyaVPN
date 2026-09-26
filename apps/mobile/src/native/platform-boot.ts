import { setAppVisibility, setClientStorage, setSystemColorSchemeReader } from "@voya/client/platform";
import { createNativeI18n } from "@voya/i18n/native";
import { Appearance, AppState } from "react-native";

import { deviceLanguages } from "./locale";
import { clientStorageAdapter, storage } from "./storage";

/**
 * Registers the React Native platform behind the shared packages.
 *
 * Imported for its side effect, first, before anything that reads a store: the
 * persisted stores are created at module scope and rehydrate as soon as their
 * module is evaluated, so registering later would let them start from the
 * in-memory fallback and then silently disagree with what is on disk.
 *
 * The desktop equivalent is `apps/desktop/src/platform-boot.ts`.
 */
setClientStorage(clientStorageAdapter);

setSystemColorSchemeReader(() => (Appearance.getColorScheme() === "dark" ? "dark" : "light"));

/**
 * What the desktop reads off `document.visibilityState`.
 *
 * Only `background` counts as off screen. `inactive` is the app switcher, a
 * notification shade or an incoming call — a blink, not a reason to tear the
 * log stream and the connection monitor down and build them again a second
 * later — and `currentState` is undefined until the first native report, which
 * must not read as "hidden" either.
 */
setAppVisibility({
  isVisible: () => AppState.currentState !== "background",
  subscribe: (onChange) => {
    const subscription = AppState.addEventListener("change", onChange);
    return () => subscription.remove();
  },
});

// MMKV's `getString`/`set` are already the shape the i18n host asks for.
const i18n = createNativeI18n({ storage, deviceLanguages });

/**
 * Settles once the startup locale's resources are in place, and never rejects.
 *
 * A stable, already-caught promise because the shell suspends on it with
 * `use()`: a new promise per render would suspend forever, and a rejection
 * would surface as a render error rather than the English fallback the failure
 * actually leaves behind.
 */
export const localeReady: Promise<void> = i18n.localeReady.catch(() => {});
