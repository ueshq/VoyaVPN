import { ClipboardPaste, ImagePlus, Monitor } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { TranslationKey } from "@voya/i18n";

export const IMPORT_METHODS = [
  { method: "clipboard", icon: ClipboardPaste, labelKey: "panes.profiles.import.clipboard" },
  { method: "qrImage", icon: ImagePlus, labelKey: "panes.profiles.importMethods.qrImage" },
  { method: "qrScreen", icon: Monitor, labelKey: "panes.profiles.importMethods.qrScreen" },
] as const satisfies readonly { method: string; icon: LucideIcon; labelKey: TranslationKey }[];

export type ImportMethod = (typeof IMPORT_METHODS)[number]["method"];

export type DirectImportMethod = Extract<ImportMethod, "clipboard" | "qrScreen">;
export type DialogImportMethod = Exclude<ImportMethod, DirectImportMethod>;
