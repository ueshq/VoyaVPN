import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";
import { cn } from "@voya/ui/lib/utils";
import { getErrorMessage } from "@voya/utils/error";
import { ModeInfo } from "./mode-info";
import { useTrafficMode } from "./use-traffic-mode";

const modes = [
  { value: "rule", labelKey: "home.trafficModeSmart" },
  { value: "global", labelKey: "proxy.trafficModeGlobal" },
] as const;

export function TrafficModeSwitcher() {
  const { t } = useI18n();
  const { disabled, error, mode, retry, selectMode } = useTrafficMode();

  return (
    <div className="home-traffic-mode">
      <div className="home-mode-row">
        <div className="home-mode-label">
          <span id="home-traffic-mode-label">{t("home.trafficMode")}</span>
          <ModeInfo
            label={t("home.trafficModeInfo")}
            hint={t("home.trafficModeHint")}
          />
        </div>
        <div
          aria-labelledby="home-traffic-mode-label"
          className="home-traffic-mode-options"
          role="group"
        >
          {modes.map(({ value, labelKey }) => (
            <Button
              aria-pressed={mode === value}
              className={cn(
                "h-8 px-4 text-xs text-foreground",
                mode === value && "bg-background text-foreground shadow-sm",
              )}
              disabled={disabled}
              key={value}
              onClick={() => selectMode(value)}
              type="button"
              variant="ghost"
            >
              {t(labelKey)}
            </Button>
          ))}
        </div>
      </div>
      {error ? (
        <div className="home-mode-hint" role="alert">
          {getErrorMessage(error)}
          <Button onClick={retry} size="sm" type="button" variant="ghost">
            {t("actions.retry")}
          </Button>
        </div>
      ) : null}
    </div>
  );
}
