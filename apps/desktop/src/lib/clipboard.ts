import { i18next } from "@voya/i18n";

/**
 * Writes text through the WebView clipboard. Some contexts do not offer it,
 * which surfaces as one translated failure instead of a `TypeError`.
 */
export async function writeClipboard(text: string) {
  if (typeof navigator === "undefined" || !navigator.clipboard?.writeText) {
    throw new Error(i18next.t("common.clipboardUnavailable"));
  }

  await navigator.clipboard.writeText(text);
}
