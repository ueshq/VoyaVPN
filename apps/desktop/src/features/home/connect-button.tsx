import { Power } from "lucide-react";

import { cn } from "@voya/ui/lib/utils";

import type { Translation } from "./use-home-runtime";

export function ConnectButton({
  label,
  busy,
  cleanupPending = false,
  connected,
  inProgress,
  onPrimaryAction,
  t,
}: {
  label?: string;
  busy: boolean;
  cleanupPending?: boolean;
  connected: boolean;
  inProgress: boolean;
  onPrimaryAction: () => void;
  t: Translation;
}) {
  const action =
    label ??
    (cleanupPending
      ? t("home.retryDisconnect")
      : connected
        ? t("actions.disconnect")
        : t("actions.connect"));
  return (
    <button
      aria-busy={busy || undefined}
      aria-label={action}
      aria-pressed={connected}
      className={cn("home-power", connected && "home-power-connected")}
      data-testid="home-connect-button"
      disabled={busy}
      onClick={onPrimaryAction}
      type="button"
    >
      {inProgress || busy ? (
        <span aria-hidden="true" className="home-power-progress" />
      ) : null}
      <Power aria-hidden="true" className="size-11" strokeWidth={1.9} />
      <span>
        {connected && !cleanupPending ? t("home.disconnectLabel") : action}
      </span>
    </button>
  );
}
