import { Switch } from "@voya/ui/components/switch";
import { Label } from "@voya/ui/components/label";

import type { Translation } from "./use-home-runtime";

/** PAC is a system-proxy preference and remains unavailable while TUN is enabled. */
export function ConnectionModeSwitcher({
  tunEnabled, modeBusy, modePending, onTunChange, onPacToggle,
  pacActive, pacAvailable, pacPending, t,
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
}) {
  return (
    <div className="home-mode-container">
      <div className="home-modes">
        <div className="home-mode">
          <Label htmlFor="home-tun-switch">{t("home.modeTun")}</Label>
          <Switch aria-busy={modePending} checked={tunEnabled} disabled={modeBusy} id="home-tun-switch" onCheckedChange={onTunChange} />
        </div>
        {!tunEnabled ? (
          <div className="home-mode">
            <Label htmlFor="home-pac-switch">{t("home.pacToggle")}</Label>
            <Switch
              aria-busy={pacPending}
              aria-describedby={pacAvailable ? undefined : "home-pac-unavailable"}
              checked={pacActive}
              disabled={modeBusy || !pacAvailable}
              id="home-pac-switch"
              onCheckedChange={onPacToggle}
            />
          </div>
        ) : null}
      </div>
      {!tunEnabled && !pacAvailable ? <p className="home-mode-hint" id="home-pac-unavailable">{t("status.sysProxyPacUnavailable")}</p> : null}
    </div>
  );
}
