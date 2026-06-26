import { Shield } from "lucide-react";

import { WindowControls } from "@/components/app-shell/window-controls";
import { useI18n } from "@/i18n/use-i18n";

/**
 * Windows-only self-drawn title bar for the borderless window: a brand mark, a
 * draggable filler, and the caption buttons. The filler carries
 * `data-tauri-drag-region` so Tauri handles move / double-click-maximize / Aero
 * Snap natively, while the brand and buttons stay outside it to stay clickable.
 *
 * It spans both shell grid columns. Every other platform keeps its native frame
 * and never mounts this (see `app-shell.tsx`). The app name lives in the sidebar
 * `h1`, so the brand text here is a plain label rather than a second heading.
 */
export function TitleBar() {
  const { t } = useI18n();

  return (
    <header className="col-span-2 flex h-10 shrink-0 items-center bg-sidebar text-sidebar-foreground select-none">
      <div className="flex shrink-0 items-center gap-2 ps-3 pe-2">
        <Shield className="size-4 text-muted-foreground" aria-hidden="true" />
        <span className="text-xs font-semibold leading-none">{t("app.name")}</span>
      </div>

      <div data-tauri-drag-region className="min-w-0 flex-1 self-stretch" />

      <WindowControls />
    </header>
  );
}
