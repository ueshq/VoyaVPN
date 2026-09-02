import i18next from "i18next";

import de from "./locales/de.json";
import en from "./locales/en.json";
import fa from "./locales/fa.json";
import fr from "./locales/fr.json";
import hu from "./locales/hu.json";
import ru from "./locales/ru.json";
import zhHans from "./locales/zh-Hans.json";
import zhHant from "./locales/zh-Hant.json";

// `nativeName` is each language's own name — locale-invariant data (never
// translated), so a language picker stays readable whatever the current UI
// language is.
export const localeOptions = [
  { code: "en", label: "EN", nativeName: "English", direction: "ltr" },
  { code: "zh-Hans", label: "简", nativeName: "简体中文", direction: "ltr" },
  { code: "zh-Hant", label: "繁", nativeName: "繁體中文", direction: "ltr" },
  { code: "fr", label: "FR", nativeName: "Français", direction: "ltr" },
  { code: "fa", label: "FA", nativeName: "فارسی", direction: "rtl" },
  { code: "hu", label: "HU", nativeName: "Magyar", direction: "ltr" },
  { code: "ru", label: "RU", nativeName: "Русский", direction: "ltr" },
  { code: "de", label: "DE", nativeName: "Deutsch", direction: "ltr" },
] as const;

export type Locale = (typeof localeOptions)[number]["code"];
export type LocaleDirection = (typeof localeOptions)[number]["direction"];
type LeafPaths<T> = {
  [Key in keyof T & string]: T[Key] extends string
    ? Key
    : T[Key] extends Record<string, unknown>
      ? `${Key}.${LeafPaths<T[Key]>}`
      : never;
}[keyof T & string];
export type TranslationKey = LeafPaths<typeof en>;
export type TranslationFunction = (key: TranslationKey, options?: Record<string, unknown>) => string;

const storageKey = "voyavpn.locale";

const i18nResources = {
  de,
  en,
  fa,
  fr,
  hu,
  ru,
  "zh-Hans": zhHans,
  "zh-Hant": zhHant,
} satisfies Record<Locale, object>;

const resources = Object.fromEntries(
  localeOptions.map(({ code }) => [code, { translation: i18nResources[code] }]),
) as Record<Locale, { translation: object }>;

function isLocale(value: string | null | undefined): value is Locale {
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

export function getLocaleDirection(locale: string | undefined): LocaleDirection {
  return localeOptions.find((option) => option.code === locale)?.direction ?? "ltr";
}

export function applyDocumentLocale(locale: Locale = getInitialLocale()) {
  if (typeof document === "undefined") {
    return;
  }

  document.documentElement.lang = locale;
  document.documentElement.dir = getLocaleDirection(locale);
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

export async function changeLocale(locale: Locale) {
  if (typeof window !== "undefined") {
    window.localStorage.setItem(storageKey, locale);
  }

  await i18next.changeLanguage(locale);
  applyDocumentLocale(locale);
}

export { i18next };
