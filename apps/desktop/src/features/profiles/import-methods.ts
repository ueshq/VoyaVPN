import type { TranslationKey } from "@voya/i18n/core";
import { ClipboardPaste, Link, Monitor } from "lucide-react";
import type { LucideIcon } from "lucide-react";

// Pasting links comes first: it takes share links and subscription URLs alike,
// which is how most nodes arrive.
export const IMPORT_METHODS = [
  { method: "paste", labelKey: "panes.profiles.importMethods.paste", icon: Link },
  { method: "clipboard", labelKey: "panes.profiles.import.clipboard", icon: ClipboardPaste },
  { method: "qrScreen", labelKey: "panes.profiles.importMethods.qrScreen", icon: Monitor },
] as const satisfies readonly { method: string; labelKey: TranslationKey; icon: LucideIcon }[];

type ImportMethod = (typeof IMPORT_METHODS)[number]["method"];

/** Runs straight away; the rest open a dialog first. */
export type DirectImportMethod = Extract<ImportMethod, "clipboard" | "qrScreen">;
export type DialogImportMethod = Exclude<ImportMethod, DirectImportMethod>;
