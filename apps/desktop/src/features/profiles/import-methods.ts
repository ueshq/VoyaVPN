import { ClipboardPaste, FileUp, ImagePlus, Monitor, TextCursorInput } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { TranslationKey } from "@voya/i18n";

export const IMPORT_METHODS = [
  { method: "clipboard", icon: ClipboardPaste, labelKey: "panes.profiles.import.clipboard" },
  { method: "text", icon: TextCursorInput, labelKey: "panes.profiles.importMethods.text" },
  { method: "file", icon: FileUp, labelKey: "panes.profiles.importMethods.file" },
  { method: "qrImage", icon: ImagePlus, labelKey: "panes.profiles.importMethods.qrImage" },
  { method: "qrClipboard", icon: ClipboardPaste, labelKey: "panes.profiles.importMethods.qrClipboard" },
  { method: "qrScreen", icon: Monitor, labelKey: "panes.profiles.importMethods.qrScreen" },
] as const satisfies readonly { method: string; icon: LucideIcon; labelKey: TranslationKey }[];

export type ImportMethod = (typeof IMPORT_METHODS)[number]["method"];

export type DirectImportMethod = Extract<ImportMethod, "clipboard" | "qrScreen">;
export type DialogImportMethod = Exclude<ImportMethod, DirectImportMethod>;
