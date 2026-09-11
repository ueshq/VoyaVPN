import type { TranslationFunction } from "@voya/i18n";

import { PROFILE_VALIDATION_CODES } from "./profile-form-schema";

/**
 * Resolve a zod issue produced by `profileFormSchema` into the locale string the
 * editor renders. Codes the schema owns are translated; anything else (zod's own
 * built-in issues) is passed through unchanged.
 */
export function profileValidationMessage(
  message: string | undefined,
  t: TranslationFunction,
): string | undefined {
  switch (message) {
    case PROFILE_VALIDATION_CODES.integerInvalid:
      return t("validation.integer");
    case PROFILE_VALIDATION_CODES.portInvalid:
      return t("validation.invalidPort");
    case PROFILE_VALIDATION_CODES.addressRequired:
      return t("panes.profiles.validation.addressRequired");
    case PROFILE_VALIDATION_CODES.credentialRequired:
      return t("panes.profiles.validation.credentialRequired");
    case PROFILE_VALIDATION_CODES.remarksRequired:
      return t("panes.profiles.validation.remarksRequired");
    case PROFILE_VALIDATION_CODES.uuidRequired:
      return t("panes.profiles.validation.uuidRequired");
    default:
      return message;
  }
}
