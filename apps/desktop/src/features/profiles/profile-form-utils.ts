import { CONFIG_TYPES, type ProfileProtocol } from "./profile-constants";
import type { TranslationFunction } from "@voya/i18n";

export function optionalNumber(value: unknown) {
  if (value === "" || value === null || value === undefined) {
    return null;
  }

  return Number(value);
}

export function passwordLabel(configType: ProfileProtocol, t: TranslationFunction) {
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

export function usernameLabel(configType: ProfileProtocol, t: TranslationFunction) {
  // The form's `username` field carries the TUIC contract's `uuid`, so it is
  // labelled UUID for that protocol.
  if (configType === CONFIG_TYPES.TUIC) {
    return t("panes.profiles.fields.uuid");
  }

  return t("panes.profiles.fields.username");
}

export function requiresUsername(configType: ProfileProtocol) {
  return configType === CONFIG_TYPES.SOCKS
    || configType === CONFIG_TYPES.HTTP
    || configType === CONFIG_TYPES.Naive
    || configType === CONFIG_TYPES.TUIC;
}
