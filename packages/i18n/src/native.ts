import { createI18nHost, LOCALE_STORAGE_KEY, type I18nSetup, type Locale } from "./core";

/**
 * The React Native host.
 *
 * This package is shared with the desktop app, so it must not import
 * `react-native` — that would break the Vite build. The caller passes the two
 * platform facts instead, from its own module that may import whatever it
 * likes.
 *
 * There is no `applyLocale` equivalent on native: no document element carries
 * the language, and every string already re-renders through `useI18n`.
 *
 * One behavioural difference from the DOM host: Metro does not code-split, so
 * the lazy `import()` of the Chinese bundles in `./core` resolves from the same
 * bundle rather than a separate chunk. `localeReady` still settles before the
 * first render, but it never waits on the network, and the startup saving the
 * DOM host gets from a separate chunk does not apply.
 */

export type NativeI18nPlatform = {
  /** A synchronous key/value store, such as MMKV. */
  storage: {
    getString: (key: string) => string | undefined;
    set: (key: string, value: string) => void;
  };
  /** The device's preferred languages, most preferred first. */
  deviceLanguages: () => readonly string[];
};

export function createNativeI18n(platform: NativeI18nPlatform): I18nSetup {
  return createI18nHost({
    readStoredLocale: () => platform.storage.getString(LOCALE_STORAGE_KEY) ?? null,
    persistLocale: (locale: Locale) => platform.storage.set(LOCALE_STORAGE_KEY, locale),
    deviceLanguages: platform.deviceLanguages,
    applyLocale: () => {},
  });
}
