import { Switch } from "@voya/ui/components/switch";
import { Button } from "@voya/ui/components/button";
import { Label } from "@voya/ui/components/label";
import type { ConnectionMode } from "@/ipc/bindings";
import { cn } from "@voya/ui/lib/utils";

import { CONNECTION_MODE_OPTIONS } from "./connection-mode";
import type { Translation } from "./use-home-runtime";

/**
 * Hiddify-style unified mode switcher: 仅代理 / 系统代理 / VPN. PAC surfaces as
 * a secondary toggle only while system proxy is active; the TUN provider
 * summary (and its error surface) shows while VPN is selected.
 */
export function ConnectionModeSwitcher({
  connectionMode,
  modeBusy,
  modePending,
  onModeChange,
  onPacToggle,
  pacActive,
  pacAvailable,
  pacPending,
  t,
  tunProviderSummary,
}: {
  connectionMode: ConnectionMode;
  modeBusy: boolean;
  modePending: ConnectionMode | null;
  onModeChange: (mode: ConnectionMode) => void;
  onPacToggle: () => void;
  pacActive: boolean;
  pacAvailable: boolean;
  pacPending: boolean;
  t: Translation;
  tunProviderSummary: string | null;
}) {
  return (
    <div className="grid justify-items-center gap-2">
      <div
        aria-busy={modePending !== null}
        aria-label={t("home.modeAria")}
        className="flex h-8 items-center rounded-lg bg-muted p-0.5"
        data-testid="home-mode-switcher"
        role="group"
      >
        {CONNECTION_MODE_OPTIONS.map((mode) => {
          const selected = connectionMode === mode;

          return (
            <Button
              key={mode}
              aria-pressed={selected}
              className={cn(
                "h-7 rounded-md px-3 text-sm leading-none shadow-none focus-visible:relative focus-visible:z-10",
                selected
                  ? "bg-background text-foreground hover:bg-background hover:text-foreground"
                  : "text-subtlest hover:bg-background/60 hover:text-foreground",
              )}
              disabled={modeBusy}
              onClick={() => onModeChange(mode)}
              size="sm"
              type="button"
              variant="ghost"
            >
              {connectionModeLabel(mode, t)}
            </Button>
          );
        })}
      </div>
      {connectionMode === "systemProxy" ? (
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
      {connectionMode === "vpn" && tunProviderSummary ? (
        <p className="max-w-md truncate text-center text-xs text-subtlest">{tunProviderSummary}</p>
      ) : null}
    </div>
  );
}

function connectionModeLabel(mode: ConnectionMode, t: Translation) {
  switch (mode) {
    case "proxyOnly":
      return t("home.modeProxyOnly");
    case "systemProxy":
      return t("home.modeSystemProxy");
    case "vpn":
    default:
      return t("home.modeVpn");
  }
}
