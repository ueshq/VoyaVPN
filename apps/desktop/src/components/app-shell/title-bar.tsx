import { useEffect, useState } from "react";
import { Copy, Minus, Square, X } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { voyaCommands } from "@voya/client/transport";
import { onWindowResized } from "@/ipc/window";
import type { TitleBarLayout } from "@voya/contracts";

/**
 * Overlays the content column, inheriting its background without occupying a
 * layout row. Only the empty filler is draggable; Windows caption buttons stay
 * clickable. macOS draws its native traffic lights above the sidebar instead.
 */
export function TitleBar({ layout }: { layout: TitleBarLayout }) {
  if (layout === "none") return null;

  return (
    <header className="shell-titlebar" data-slot="titlebar">
      <div data-tauri-drag-region className="min-w-0 flex-1 self-stretch" />
      {layout === "windows" && <WindowControls />}
    </header>
  );
}

/**
 * The three caption buttons (minimize / maximize-restore / close) for the
 * Windows borderless title bar. They drive the current window through the
 * window commands and deliberately sit outside the drag region so every click
 * registers. The maximize glyph swaps to a restore glyph while maximized.
 */
function WindowControls() {
  const { t } = useI18n();
  const [maximized, setMaximized] = useState(false);

  // Mirror the live maximized state into the icon: read it once on mount, then
  // refresh on every resize so Aero Snap and double-click-to-maximize stay in
  // sync without us tracking the toggle ourselves.
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    // A failed read or a failed subscription leaves the icon as it was; the
    // buttons still work, so neither is worth more than not rejecting unhandled.
    const sync = () => {
      void voyaCommands()
        .isWindowMaximized()
        .then((value) => {
          if (active) setMaximized(value);
        })
        .catch(() => undefined);
    };

    sync();
    void onWindowResized(sync)
      .then((fn) => {
        if (active) unlisten = fn;
        else fn();
      })
      .catch(() => undefined);

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
        onClick={() => void voyaCommands().minimizeWindow().catch(() => undefined)}
        type="button"
      >
        <Minus className="size-4" aria-hidden="true" />
      </button>
      <button
        aria-label={maximized ? t("window.restore") : t("window.maximize")}
        className={buttonClass}
        onClick={() => void voyaCommands().toggleMaximizeWindow().catch(() => undefined)}
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
        onClick={() => void voyaCommands().closeWindow().catch(() => undefined)}
        type="button"
      >
        <X className="size-4" aria-hidden="true" />
      </button>
    </div>
  );
}
