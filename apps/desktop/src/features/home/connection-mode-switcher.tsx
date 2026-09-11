import { Switch } from "@voya/ui/components/switch";
import { Label } from "@voya/ui/components/label";

import type { TranslationFunction } from "@voya/i18n";

export function ConnectionModeSwitcher({
  tunEnabled,
  modeBusy,
  modePending,
  onTunChange,
  t,
}: {
  tunEnabled: boolean;
  modeBusy: boolean;
  modePending: boolean;
  onTunChange: (enabled: boolean) => void;
  t: TranslationFunction;
}) {
  return (
    <div className="home-mode-container">
      <div className="home-modes">
        <div className="home-mode">
          <Label htmlFor="home-tun-switch">{t("home.modeTun")}</Label>
          <Switch
            aria-busy={modePending}
            checked={tunEnabled}
            disabled={modeBusy}
            id="home-tun-switch"
            onCheckedChange={onTunChange}
          />
        </div>
      </div>
      <p className="home-mode-hint">{t("home.tunHint")}</p>
    </div>
  );
}
