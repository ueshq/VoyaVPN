import { NativeModules, Platform } from "react-native";

/**
 * The device's preferred languages, most preferred first.
 *
 * Read from the platform's own settings rather than a library: this is the only
 * fact needed, both platforms expose it on a module React Native already links,
 * and `@voya/i18n` turns the tags into a supported locale.
 */
export function deviceLanguages(): readonly string[] {
  if (Platform.OS === "ios") {
    const settings = NativeModules.SettingsManager?.settings as
      | { AppleLanguages?: string[]; AppleLocale?: string }
      | undefined;

    const languages = settings?.AppleLanguages;
    if (Array.isArray(languages) && languages.length > 0) {
      return languages;
    }

    return settings?.AppleLocale ? [settings.AppleLocale] : [];
  }

  const locale = (NativeModules.I18nManager?.localeIdentifier as string | undefined) ?? "";

  // Android reports an underscore-separated identifier (`zh_Hant_TW`); language
  // tags are hyphen-separated.
  return locale ? [locale.replace(/_/g, "-")] : [];
}
