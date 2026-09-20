/**
 * The desktop half of the app-visibility seam in `@voya/client`.
 *
 * `false` while the window is hidden into the tray or minimized, which is when
 * live streams should stop feeding it. `useAppVisible` is the shared hook that
 * reads this; nothing subscribes to the document directly.
 */
export function subscribeToDocumentVisible(onChange: () => void) {
  document.addEventListener("visibilitychange", onChange);
  return () => document.removeEventListener("visibilitychange", onChange);
}

export function isDocumentVisible() {
  return document.visibilityState !== "hidden";
}
