import type { ProfileKind } from "@voya/contracts";
import type { TranslationFunction } from "@voya/i18n/core";

export function optionalNumber(value: unknown) {
  if (value === "" || value === null || value === undefined) {
    return null;
  }

  return Number(value);
}

export function passwordLabel(configType: ProfileKind, t: TranslationFunction) {
  // TUIC is deliberately absent: it carries a UUID *and* a password, and the
  // UUID is edited through the username input (see `usernameLabel`).
  if (configType === "vmess" || configType === "vless") {
    return t("panes.profiles.fields.uuid");
  }
  if (configType === "wireGuard") {
    return t("panes.profiles.fields.privateKey");
  }

  return t("panes.profiles.fields.password");
}

export function usernameLabel(configType: ProfileKind, t: TranslationFunction) {
  // The form's `username` field carries the TUIC contract's `uuid`, so it is
  // labelled UUID for that protocol.
  if (configType === "tuic") {
    return t("panes.profiles.fields.uuid");
  }

  return t("panes.profiles.fields.username");
}

export function requiresUsername(configType: ProfileKind) {
  return configType === "socks"
    || configType === "http"
    || configType === "naive"
    || configType === "tuic";
}
