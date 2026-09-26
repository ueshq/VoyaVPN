import { ClipboardPaste, Link, Monitor } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import type { ImportMethod } from "@/features/profiles/import-methods";

/** The desktop icon for each shared import method. */
export const IMPORT_METHOD_ICONS: Record<ImportMethod, LucideIcon> = {
  clipboard: ClipboardPaste,
  paste: Link,
  qrScreen: Monitor,
};
