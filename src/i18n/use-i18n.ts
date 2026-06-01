import { useSyncExternalStore } from "react";

import {
  changeLocale,
  getLocaleDirection,
  i18next,
  localeOptions,
  type Locale,
} from "@/i18n";

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

  return {
    direction: getLocaleDirection(language),
    language,
    localeOptions,
    setLocale: changeLocale,
    t: (key: string, options?: Record<string, unknown>) => String(i18next.t(key, options)),
  };
}
