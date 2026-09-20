import { useMemo, useSyncExternalStore } from "react";

// `./core`, not `./index`: the hook is platform-free and must not drag the DOM
// host into a React Native bundle.
import { i18next, localeOptions, type Locale, type TranslationFunction } from "./core";

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
    t,
  };
}
