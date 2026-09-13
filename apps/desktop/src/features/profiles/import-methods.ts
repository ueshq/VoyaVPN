import { ClipboardPaste, Link, Monitor } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { TranslationKey } from "@voya/i18n";

// Pasting links comes first: it takes share links and subscription URLs alike,
// which is how most nodes arrive.
export const IMPORT_METHODS = [
  { method: "paste", icon: Link, labelKey: "panes.profiles.importMethods.paste" },
  { method: "clipboard", icon: ClipboardPaste, labelKey: "panes.profiles.import.clipboard" },
  { method: "qrScreen", icon: Monitor, labelKey: "panes.profiles.importMethods.qrScreen" },
] as const satisfies readonly { method: string; icon: LucideIcon; labelKey: TranslationKey }[];

export type ImportMethod = (typeof IMPORT_METHODS)[number]["method"];

export type DirectImportMethod = Extract<ImportMethod, "clipboard" | "qrScreen">;
export type DialogImportMethod = Exclude<ImportMethod, DirectImportMethod>;
