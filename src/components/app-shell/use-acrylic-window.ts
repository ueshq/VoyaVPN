import { useEffect } from "react";

import { setWindowAcrylic } from "@/ipc";

/** CSS scope hook class: marks "this window has Windows Acrylic enabled" so the veil overrides in globals.css match. */
const ACRYLIC_SCOPE_CLASS = "voyavpn-acrylic";

/**
 * Wire up the Windows Acrylic blur material (desktop Windows only).
 * - Adds `voyavpn-acrylic` to `<html>` so the scoped veil styles take effect
 *   (web / macOS never match and keep the flat neutral surfaces).
 * - Watches the `.dark` class on `<html>` (the in-app light/dark source of truth)
 *   and re-tints the native material via `set_window_acrylic` so the OS blur's base
 *   color stays aligned with the UI theme. Failures stay silent (e.g. the user has
 *   transparency effects disabled system-wide).
 */
export function useAcrylicWindow(enabled: boolean): void {
  useEffect(() => {
    if (!enabled) return undefined;

    const root = document.documentElement;
    root.classList.add(ACRYLIC_SCOPE_CLASS);

    const syncTheme = () => {
      void setWindowAcrylic(root.classList.contains("dark")).catch(() => undefined);
    };
    syncTheme();

    const observer = new MutationObserver(syncTheme);
    observer.observe(root, { attributes: true, attributeFilter: ["class"] });

    return () => {
      observer.disconnect();
      root.classList.remove(ACRYLIC_SCOPE_CLASS);
    };
  }, [enabled]);
}
