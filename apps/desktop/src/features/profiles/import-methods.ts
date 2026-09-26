import type { TranslationKey } from "@voya/i18n/core";

// Pasting links comes first: it takes share links and subscription URLs alike,
// which is how most nodes arrive.
export const IMPORT_METHODS = [
  { method: "paste", labelKey: "panes.profiles.importMethods.paste" },
  { method: "clipboard", labelKey: "panes.profiles.import.clipboard" },
  { method: "qrScreen", labelKey: "panes.profiles.importMethods.qrScreen" },
] as const satisfies readonly { method: string; labelKey: TranslationKey }[];

export type ImportMethod = (typeof IMPORT_METHODS)[number]["method"];

/** Runs straight away; the rest open a dialog first. */
export type DirectImportMethod = Extract<ImportMethod, "clipboard" | "qrScreen">;
export type DialogImportMethod = Exclude<ImportMethod, DirectImportMethod>;
