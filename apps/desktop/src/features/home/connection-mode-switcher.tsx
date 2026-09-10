import { Switch } from "@voya/ui/components/switch";
import { Label } from "@voya/ui/components/label";
import { cn } from "@voya/ui/lib/utils";

import type { Translation } from "./use-home-runtime";

/** TUN toggle with PAC settings for system proxy and active TUN diagnostics. */
export function ConnectionModeSwitcher({
  tunEnabled,
  modeBusy,
  modePending,
  onTunChange,
  onPacToggle,
  pacActive,
  pacAvailable,
  pacPending,
  t,
  tunProviderSummary,
}: {
  tunEnabled: boolean;
  modeBusy: boolean;
  modePending: boolean;
  onTunChange: (enabled: boolean) => void;
  onPacToggle: () => void;
  pacActive: boolean;
  pacAvailable: boolean;
  pacPending: boolean;
  t: Translation;
  tunProviderSummary: string | null;
}) {
  return (
    <div className="grid justify-items-center gap-2">
      <div className="flex h-8 items-center gap-2">
        <Label htmlFor="home-tun-switch">{t("home.modeTun")}</Label>
        <Switch
          aria-busy={modePending}
          checked={tunEnabled}
          disabled={modeBusy}
          id="home-tun-switch"
          onCheckedChange={onTunChange}
        />
      </div>
      {!tunEnabled ? (
        <div className="flex items-center gap-2">
          <Label
            className={cn("text-xs", pacAvailable ? "text-muted-foreground" : "text-subtlest")}
            htmlFor="home-pac-switch"
            id="home-pac-label"
          >
            {t("home.pacToggle")}
          </Label>
          <Switch
            aria-busy={pacPending}
            aria-describedby={pacAvailable ? undefined : "home-pac-unavailable"}
            checked={pacActive}
            disabled={modeBusy || !pacAvailable}
            id="home-pac-switch"
            onCheckedChange={onPacToggle}
          />
          {!pacAvailable ? (
            <p className="text-xs text-subtlest" id="home-pac-unavailable">
              {t("status.sysProxyPacUnavailable")}
            </p>
          ) : null}
        </div>
      ) : null}
      {tunEnabled && tunProviderSummary ? (
        <p className="max-w-md text-center text-xs text-subtlest">{tunProviderSummary}</p>
      ) : null}
    </div>
  );
}
