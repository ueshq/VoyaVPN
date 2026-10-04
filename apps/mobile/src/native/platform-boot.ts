import { focusManager } from "@tanstack/react-query";
import { setAppVisibility, setClientStorage } from "@voya/client/platform";
import { setRuntimeChannels } from "@voya/client/runtime-status";
import { createNativeI18n } from "@voya/i18n/native";
import { AppState } from "react-native";

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

/**
 * What the desktop reads off `document.visibilityState`.
 *
 * Only `background` counts as off screen. `inactive` is the app switcher, a
 * notification shade or an incoming call — a blink, not a reason to tear the
 * log stream and the connection monitor down and build them again a second
 * later — and `currentState` is undefined until the first native report, which
 * must not read as "hidden" either.
 */
const appIsVisible = () => AppState.currentState !== "background";

setAppVisibility({
  isVisible: appIsVisible,
  // Subscribers are told when visibility changes, not when the app state
  // does: active and inactive are both "visible", and passing the blink
  // between them on re-read the whole runtime status twice for every pull
  // of the notification shade.
  subscribe: (onChange) => {
    let visible = appIsVisible();
    const subscription = AppState.addEventListener("change", () => {
      if (appIsVisible() === visible) return;
      visible = !visible;
      onChange();
    });
    return () => subscription.remove();
  },
});

/**
 * TanStack Query's idea of "focused", which a phone has to be told about.
 *
 * Left alone it reads every React Native app as focused forever, so a polled
 * query — the running policy group, every three seconds — kept polling behind
 * the lock screen, for as long as Android's VPN service kept the process
 * alive. Polling pauses while unfocused; refetch-on-focus is off for every
 * query but the exit address, so coming back causes no burst. The same rule as the visibility above: only
 * `background` counts.
 */
focusManager.setEventListener((setFocused) => {
  const subscription = AppState.addEventListener("change", (state) => {
    setFocused(state !== "background");
  });
  return () => subscription.remove();
});

/**
 * A phone has no system proxy to read and no tunnel status apart from the
 * core's: the provider is the core. Asking for either after a connect would
 * raise an "unsupported" failure every time.
 */
setRuntimeChannels(["coreState"]);

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
