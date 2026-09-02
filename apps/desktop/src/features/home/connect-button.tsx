import { Power } from "lucide-react";

import { cn } from "@voya/ui/lib/utils";

import type { Translation } from "./use-home-runtime";

/**
 * Hiddify-style central connect control. The accessible name stays a constant
 * "Connect"; state travels via `aria-pressed`/`aria-busy`. Single-accent
 * discipline: idle stays neutral, affirmative green (`--connected` /
 * `--connected-glow`) marks only the achieved protected state, and the
 * connecting ring inherits the global `prefers-reduced-motion` guard.
 */
export function ConnectButton({
  busy,
  connected,
  inProgress,
  onPrimaryAction,
  t,
}: {
  busy: boolean;
  connected: boolean;
  inProgress: boolean;
  onPrimaryAction: () => void;
  t: Translation;
}) {
  return (
    <button
      aria-busy={busy || undefined}
      aria-label={t("actions.connect")}
      aria-pressed={connected}
      className={cn(
        "relative grid size-36 place-items-center rounded-full border-2 outline-none transition-colors",
        "focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-70",
        connected
          ? "border-connected/60 bg-connected/10 text-connected shadow-[var(--connected-glow)]"
          : "border-border bg-surface-sunken text-muted-foreground hover:border-ring hover:text-foreground",
      )}
      data-testid="home-connect-button"
      disabled={busy}
      onClick={onPrimaryAction}
      type="button"
    >
      {inProgress || busy ? (
        <span
          aria-hidden="true"
          className="absolute inset-[-2px] animate-spin rounded-full border-2 border-transparent border-t-brand"
        />
      ) : null}
      <Power aria-hidden="true" className="size-12" strokeWidth={1.75} />
    </button>
  );
}
