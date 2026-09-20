import i18next from "i18next";

import en from "./locales/en.json";

/**
 * The platform-free half of the i18n setup.
 *
 * Nothing here reads `window`, `navigator` or `document`. The one import-time
 * side effect is initialising `i18next` in English, so that importing any entry
 * of this package yields a working translator (see the `init` call below).
 *
 * A host module supplies the platform facts — where the choice is stored, what
 * the device's languages are, and how to apply a locale to the UI shell — and
 * calls `createI18nHost` once. `./index` is the DOM host the desktop app uses;
 * `./native` is the React Native one.
 */

// `nativeName` is each language's own name — locale-invariant data (never
// translated), so a language picker stays readable whatever the current UI
// language is.
export const localeOptions = [
  { code: "en", label: "EN", nativeName: "English" },
  { code: "zh-Hans", label: "简", nativeName: "简体中文" },
  { code: "zh-Hant", label: "繁", nativeName: "繁體中文" },
] as const;

export type Locale = (typeof localeOptions)[number]["code"];
type LeafPaths<T> = {
  [Key in keyof T & string]: T[Key] extends string
    ? Key
    : T[Key] extends Record<string, unknown>
      ? `${Key}.${LeafPaths<T[Key]>}`
      : never;
}[keyof T & string];
export type TranslationKey = LeafPaths<typeof en>;
export type TranslationFunction = (key: TranslationKey, options?: Record<string, unknown>) => string;

/** Storage key holding the chosen locale. `changeLocale` is the only writer. */
export const LOCALE_STORAGE_KEY = "voyavpn.locale";

/**
 * English ships with the entry: it is the fallback for every key and the
 * source of `TranslationKey`. The others load when first chosen, so a UI in
 * one language does not parse the other two at startup.
 */
const lazyLocales: Record<Exclude<Locale, "en">, () => Promise<{ default: object }>> = {
  "zh-Hans": () => import("./locales/zh-Hans.json"),
  "zh-Hant": () => import("./locales/zh-Hant.json"),
};

async function loadLocale(locale: Locale) {
  if (locale === "en" || i18next.hasResourceBundle(locale, "translation")) {
    return;
  }

  const { default: translation } = await lazyLocales[locale]();
  i18next.addResourceBundle(locale, "translation", translation);
}

export function isLocale(value: string | null | undefined): value is Locale {
  return localeOptions.some((locale) => locale.code === value);
}

/**
 * Picks the best supported locale out of a device's ordered language tags.
 *
 * Chinese needs the script, not the base language: `zh-TW`/`zh-HK`/`zh-MO` are
 * traditional while a bare `zh` is simplified. Everything else falls back to
 * the base subtag.
 */
export function localeFromLanguageTags(tags: readonly string[]): Locale | undefined {
  for (const tag of tags) {
    const normalized = tag.toLowerCase();

    if (normalized.startsWith("zh-hant") || ["zh-tw", "zh-hk", "zh-mo"].includes(normalized)) {
      return "zh-Hant";
    }

    if (normalized.startsWith("zh")) {
      return "zh-Hans";
    }

    const baseLanguage = normalized.split("-")[0];

    if (isLocale(baseLanguage)) {
      return baseLanguage;
    }
  }

  return undefined;
}

/** The platform facts the setup cannot observe for itself. */
export type I18nHost = {
  /** The stored choice, or `null`/`undefined` when there is none. */
  readStoredLocale: () => string | null | undefined;
  /** Persists a chosen locale. Only called when `changeLocale` is asked to persist. */
  persistLocale: (locale: Locale) => void;
  /** The device's preferred languages, most preferred first. */
  deviceLanguages: () => readonly string[];
  /** Applies a locale to the UI shell — `<html lang>` on the web, a no-op on native. */
  applyLocale: (locale: Locale) => void;
};

export type I18nSetup = {
  getInitialLocale: () => Locale;
  applyLocale: (locale?: Locale) => void;
  changeLocale: (locale: Locale, options?: { persist?: boolean }) => Promise<void>;
  /**
   * Settles once the startup locale's resources are in place. An entry point
   * renders after it, so a Chinese UI never flashes English first; a failed
   * load leaves the English fallback.
   */
  localeReady: Promise<void>;
};

/**
 * The base initialisation, in English, at import time.
 *
 * Importing any entry of this package has always yielded a working `i18next`,
 * and `useI18n` relies on it: a component can render translated text without
 * the app having wired up a host. A host then layers detection and persistence
 * on top by switching the language, so this stays the only `init` call.
 */
void i18next.init({
  resources: { en: { translation: en } },
  lng: "en",
  fallbackLng: "en",
  initAsync: false,
  returnNull: false,
  supportedLngs: localeOptions.map((locale) => locale.code),
  interpolation: {
    escapeValue: false,
  },
});

/**
 * Wires one host's platform facts to the shared `i18next` singleton and returns
 * the locale controls. Call this once per process.
 */
export function createI18nHost(host: I18nHost): I18nSetup {
  const getInitialLocale = (): Locale => {
    const storedLocale = host.readStoredLocale();

    if (isLocale(storedLocale)) {
      return storedLocale;
    }

    return localeFromLanguageTags(host.deviceLanguages()) ?? "en";
  };

  const applyLocale = (locale: Locale = getInitialLocale()) => {
    host.applyLocale(locale);
  };

  const initialLocale = getInitialLocale();

  applyLocale(initialLocale);

  /**
   * Switch the UI language.
   *
   * `persist: false` previews a locale for the current session only — the
   * Settings surface needs that so an unsaved appearance edit never becomes the
   * stored preference. Without it a consumer has to re-declare this module's
   * private storage key and snapshot/restore storage around the call.
   */
  const changeLocale = async (locale: Locale, options?: { persist?: boolean }) => {
    if (options?.persist !== false) {
      host.persistLocale(locale);
    }

    await loadLocale(locale);
    await i18next.changeLanguage(locale);
    applyLocale(locale);
  };

  const localeReady: Promise<void> =
    initialLocale === "en" ? Promise.resolve() : changeLocale(initialLocale, { persist: false });

  const setup: I18nSetup = { getInitialLocale, applyLocale, changeLocale, localeReady };
  active = setup;

  return setup;
}

/**
 * The host this process wired up, if any.
 *
 * Shared code has to change the language without knowing whether it is running
 * in a WebView or in React Native — the Settings surface previews a locale and
 * then saves or discards it. Before a host exists, changing the language is a
 * no-op rather than a crash: a store may be read while the app is still
 * starting, and the initial locale is applied by `createI18nHost` anyway.
 */
let active: I18nSetup = {
  applyLocale: () => {},
  changeLocale: async () => {},
  getInitialLocale: () => "en",
  localeReady: Promise.resolve(),
};

export function i18nHost(): I18nSetup {
  return active;
}

export { i18next };
