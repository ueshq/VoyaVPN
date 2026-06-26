import { useEffect, useState } from "react";
import { Copy, Minus, Square, X } from "lucide-react";

import { useI18n } from "@/i18n/use-i18n";
import {
  closeWindow,
  isWindowMaximized,
  minimizeWindow,
  onWindowResized,
  toggleMaximizeWindow,
} from "@/ipc/window";

/**
 * The three caption buttons (minimize / maximize-restore / close) for the
 * Windows borderless title bar. They drive the current window straight through
 * `@/ipc/window` and deliberately sit outside the drag region so every click
 * registers. The maximize glyph swaps to a restore glyph while maximized.
 */
export function WindowControls() {
  const { t } = useI18n();
  const [maximized, setMaximized] = useState(false);

  // Mirror the live maximized state into the icon: read it once on mount, then
  // refresh on every resize so Aero Snap and double-click-to-maximize stay in
  // sync without us tracking the toggle ourselves.
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    const sync = () => {
      void isWindowMaximized().then((value) => {
        if (active) setMaximized(value);
      });
    };

    sync();
    void onWindowResized(sync).then((fn) => {
      if (active) unlisten = fn;
      else fn();
    });

    return () => {
      active = false;
      unlisten?.();
    };
  }, []);

  const buttonClass =
    "flex h-full w-12 items-center justify-center text-foreground/70 transition-colors hover:bg-overlay-hovered";

  return (
    <div className="flex items-stretch self-stretch">
      <button
        aria-label={t("window.minimize")}
        className={buttonClass}
        onClick={() => void minimizeWindow()}
        type="button"
      >
        <Minus className="size-4" aria-hidden="true" />
      </button>
      <button
        aria-label={maximized ? t("window.restore") : t("window.maximize")}
        className={buttonClass}
        onClick={() => void toggleMaximizeWindow()}
        type="button"
      >
        {maximized ? (
          <Copy className="size-3.5" aria-hidden="true" />
        ) : (
          <Square className="size-3.5" aria-hidden="true" />
        )}
      </button>
      <button
        aria-label={t("window.close")}
        className="flex h-full w-12 items-center justify-center text-foreground/70 transition-colors hover:bg-destructive hover:text-white"
        onClick={() => void closeWindow()}
        type="button"
      >
        <X className="size-4" aria-hidden="true" />
      </button>
    </div>
  );
}
