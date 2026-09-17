/** Restore focus to the opener, or to `fallback` when the opener was removed. */
export function restoreFocus(
  trigger: HTMLElement | null | undefined,
  fallback: HTMLElement | null,
) {
  (trigger?.isConnected ? trigger : fallback)?.focus();
}
