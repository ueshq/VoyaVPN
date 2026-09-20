import { useSyncExternalStore } from "react";

function subscribeToVisibility(onChange: () => void) {
  document.addEventListener("visibilitychange", onChange);
  return () => document.removeEventListener("visibilitychange", onChange);
}

function isDocumentVisible() {
  return document.visibilityState !== "hidden";
}

/**
 * Whether the document is on screen: `false` while the window is hidden into
 * the tray or minimized, which is when live streams should stop feeding it.
 */
export function useDocumentVisible() {
  return useSyncExternalStore(subscribeToVisibility, isDocumentVisible);
}
