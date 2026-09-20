import { createI18nHost, LOCALE_STORAGE_KEY, type Locale } from "./core";

/**
 * The DOM host.
 *
 * Importing this module initialises i18next for a browser or webview: the
 * choice persists in `localStorage`, detection reads `navigator.languages`, and
 * applying a locale writes `<html lang>`. React Native imports `./native`
 * instead; both sit on the same `./core`.
 */
const setup = createI18nHost({
  readStoredLocale: () =>
    typeof window === "undefined" ? undefined : window.localStorage.getItem(LOCALE_STORAGE_KEY),
  persistLocale: (locale) => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(LOCALE_STORAGE_KEY, locale);
    }
  },
  deviceLanguages: () => {
    if (typeof navigator === "undefined") {
      return [];
    }

    return navigator.languages.length > 0 ? navigator.languages : [navigator.language];
  },
  applyLocale: (locale) => {
    if (typeof document === "undefined") {
      return;
    }

    document.documentElement.lang = locale;
    document.documentElement.dir = "ltr";
  },
});

export const { getInitialLocale, changeLocale, localeReady } = setup;

/** Writes the locale onto `<html>`; named for what it does on this platform. */
export const applyDocumentLocale: (locale?: Locale) => void = setup.applyLocale;

// `localeOptions` is not re-exported: components read it from `useI18n()`,
// and anything outside React takes it from `@voya/i18n/core`.
export { i18next, isLocale } from "./core";
export type { Locale, TranslationFunction, TranslationKey } from "./core";
