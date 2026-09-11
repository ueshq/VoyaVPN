import i18next from "i18next";

import en from "./locales/en.json";
import zhHans from "./locales/zh-Hans.json";
import zhHant from "./locales/zh-Hant.json";

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

// localStorage key holding the chosen locale. `changeLocale` is the only writer.
const storageKey = "voyavpn.locale";

const i18nResources = {
  en,
  "zh-Hans": zhHans,
  "zh-Hant": zhHant,
} satisfies Record<Locale, object>;

const resources = Object.fromEntries(
  localeOptions.map(({ code }) => [code, { translation: i18nResources[code] }]),
) as Record<Locale, { translation: object }>;

export function isLocale(value: string | null | undefined): value is Locale {
  return localeOptions.some((locale) => locale.code === value);
}

function readStoredLocale() {
  if (typeof window === "undefined") {
    return undefined;
  }

  return window.localStorage.getItem(storageKey);
}

function getBrowserLocale() {
  if (typeof navigator === "undefined") {
    return undefined;
  }

  const languages = navigator.languages.length > 0 ? navigator.languages : [navigator.language];

  for (const language of languages) {
    const normalized = language.toLowerCase();

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

  return "en";
}

export function getInitialLocale(): Locale {
  const storedLocale = readStoredLocale();

  if (isLocale(storedLocale)) {
    return storedLocale;
  }

  return getBrowserLocale() ?? "en";
}

export function applyDocumentLocale(locale: Locale = getInitialLocale()) {
  if (typeof document === "undefined") {
    return;
  }

  document.documentElement.lang = locale;
  document.documentElement.dir = "ltr";
}

void i18next.init({
  resources,
  lng: getInitialLocale(),
  fallbackLng: "en",
  initAsync: false,
  returnNull: false,
  supportedLngs: localeOptions.map((locale) => locale.code),
  interpolation: {
    escapeValue: false,
  },
});

applyDocumentLocale(i18next.resolvedLanguage as Locale);

/**
 * Switch the UI language.
 *
 * `persist: false` previews a locale for the current session only — the Settings
 * surface needs that so an unsaved appearance edit never becomes the stored
 * preference. Without it a consumer has to re-declare this module's private
 * storage key and snapshot/restore localStorage around the call.
 */
export async function changeLocale(locale: Locale, options?: { persist?: boolean }) {
  if (options?.persist !== false && typeof window !== "undefined") {
    window.localStorage.setItem(storageKey, locale);
  }

  await i18next.changeLanguage(locale);
  applyDocumentLocale(locale);
}

export { i18next };
