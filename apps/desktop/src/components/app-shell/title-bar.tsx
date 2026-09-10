import { WindowControls } from "@/components/app-shell/window-controls";
import type { TitleBarLayout } from "@/ipc/bindings";

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
