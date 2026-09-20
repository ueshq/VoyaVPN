import type { ZodError } from "zod";

import { i18next, type TranslationFunction, type TranslationKey } from "@voya/i18n/core";

/**
 * Field path → translation key for a rejected form value.
 *
 * Zod messages are rendered verbatim into field-error spans, so a hardcoded
 * English `message:` would bypass the locale system entirely (the i18n gate only
 * scans JSX text and cannot see them). Every schema in this app therefore uses a
 * translation key as its message and the rendering surface translates it here.
 */
export type FieldErrorMap = Record<string, TranslationKey>;

/** Shown when an issue carries a message Zod produced itself. */
const GENERIC_ERROR_KEY: TranslationKey = "validation.invalid";

export function zodIssuesToErrorMap(error: ZodError): FieldErrorMap {
  return Object.fromEntries(
    error.issues.map((issue) => [issue.path.join(".") || "form", toTranslationKey(issue.message)]),
  );
}

/**
 * Zod's own messages (a failed `z.enum`, say) are plain English and must not
 * reach the UI; only messages that name a real locale key are kept.
 */
function toTranslationKey(message: string): TranslationKey {
  return localeKeyExists(message) ? (message as TranslationKey) : GENERIC_ERROR_KEY;
}

// Wrapping the call deliberately drops i18next's type-guard narrowing: it
// narrows to i18next's own key type, not to Voya's `TranslationKey`.
function localeKeyExists(key: string): boolean {
  return i18next.exists(key);
}

/** Renders a field-error map for a surface whose props take plain strings. */
export function translateFieldErrors(
  t: TranslationFunction,
  errors: FieldErrorMap,
): Record<string, string> {
  return Object.fromEntries(Object.entries(errors).map(([field, key]) => [field, t(key)]));
}
