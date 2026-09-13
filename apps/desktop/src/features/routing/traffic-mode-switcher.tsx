import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";
import { cn } from "@voya/ui/lib/utils";

import { ModeInfo } from "./mode-info";
import { useTrafficMode } from "./use-traffic-mode";

const MODES = [
  { value: "rule", labelKey: "panes.routing.trafficModeRule" },
  { value: "global", labelKey: "proxy.trafficModeGlobal" },
] as const;

/**
 * Rule or global mode, in the Rules page title. A read failure is reported by
 * the banner below the title, which also explains the global lock.
 */
export function TrafficModeSwitcher() {
  const { t } = useI18n();
  const { disabled, mode, selectMode } = useTrafficMode();

  return (
    <div className="flex items-center gap-2">
      <span className="inline-flex items-center gap-1 text-sm text-muted-foreground">
        <span id="routing-traffic-mode-label">{t("panes.routing.trafficMode")}</span>
        <ModeInfo
          hint={t("panes.routing.trafficModeHint")}
          label={t("panes.routing.trafficModeInfo")}
        />
      </span>
      <div
        aria-labelledby="routing-traffic-mode-label"
        className="flex h-8 w-fit items-center rounded-lg bg-muted p-0.5"
        role="group"
      >
        {MODES.map(({ value, labelKey }) => (
          <Button
            aria-pressed={mode === value}
            className={cn(
              "h-7 rounded-md px-3 text-sm leading-none text-foreground shadow-none focus-visible:relative focus-visible:z-10",
              mode === value
                ? "bg-background hover:bg-background"
                : "hover:bg-background/60",
            )}
            disabled={disabled}
            key={value}
            onClick={() => selectMode(value)}
            size="sm"
            type="button"
            variant="ghost"
          >
            {t(labelKey)}
          </Button>
        ))}
      </div>
    </div>
  );
}
