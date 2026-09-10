import { useMemo, useSyncExternalStore } from "react";

import {
  changeLocale,
  i18next,
  localeOptions,
  type Locale,
  type TranslationFunction,
} from "./index";

function subscribe(listener: () => void) {
  i18next.on("languageChanged", listener);

  return () => {
    i18next.off("languageChanged", listener);
  };
}

function getSnapshot() {
  return i18next.resolvedLanguage ?? i18next.language;
}

export function useI18n() {
  const language = useSyncExternalStore(subscribe, getSnapshot, getSnapshot) as Locale;
  const t = useMemo<TranslationFunction>(() => {
    const fixedT = i18next.getFixedT(language);
    return (key, options) => String(fixedT(key, options));
  }, [language]);

  return {
    language,
    localeOptions,
    setLocale: changeLocale,
    t,
  };
}
