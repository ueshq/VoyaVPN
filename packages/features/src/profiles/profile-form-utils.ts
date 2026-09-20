import type { ProfileKind } from "@voya/contracts";
import { CONFIG_TYPES } from "@voya/features/profiles/profile-constants";
import type { TranslationFunction } from "@voya/i18n";
import { PROFILE_VALIDATION_CODES } from "./profile-form-schema";

export function optionalNumber(value: unknown) {
  if (value === "" || value === null || value === undefined) {
    return null;
  }

  return Number(value);
}

export function passwordLabel(configType: ProfileKind, t: TranslationFunction) {
  // TUIC is deliberately absent: it carries a UUID *and* a password, and the
  // UUID is edited through the username input (see `usernameLabel`).
  if (configType === CONFIG_TYPES.VMess || configType === CONFIG_TYPES.VLESS) {
    return t("panes.profiles.fields.uuid");
  }
  if (configType === CONFIG_TYPES.WireGuard) {
    return t("panes.profiles.fields.privateKey");
  }

  return t("panes.profiles.fields.password");
}

export function usernameLabel(configType: ProfileKind, t: TranslationFunction) {
  // The form's `username` field carries the TUIC contract's `uuid`, so it is
  // labelled UUID for that protocol.
  if (configType === CONFIG_TYPES.TUIC) {
    return t("panes.profiles.fields.uuid");
  }

  return t("panes.profiles.fields.username");
}

export function requiresUsername(configType: ProfileKind) {
  return configType === CONFIG_TYPES.SOCKS
    || configType === CONFIG_TYPES.HTTP
    || configType === CONFIG_TYPES.Naive
    || configType === CONFIG_TYPES.TUIC;
}

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
