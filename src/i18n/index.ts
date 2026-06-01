import i18next from "i18next";

import de from "./locales/de.json";
import en from "./locales/en.json";
import fa from "./locales/fa.json";
import fr from "./locales/fr.json";
import hu from "./locales/hu.json";
import ru from "./locales/ru.json";
import zhHans from "./locales/zh-Hans.json";
import zhHant from "./locales/zh-Hant.json";

export const localeOptions = [
  { code: "en", label: "EN", direction: "ltr" },
  { code: "zh-Hans", label: "简", direction: "ltr" },
  { code: "zh-Hant", label: "繁", direction: "ltr" },
  { code: "fr", label: "FR", direction: "ltr" },
  { code: "fa", label: "FA", direction: "rtl" },
  { code: "hu", label: "HU", direction: "ltr" },
  { code: "ru", label: "RU", direction: "ltr" },
  { code: "de", label: "DE", direction: "ltr" },
] as const;

export type Locale = (typeof localeOptions)[number]["code"];
export type LocaleDirection = (typeof localeOptions)[number]["direction"];

const storageKey = "voyavpn.locale";

export const i18nResources = {
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
