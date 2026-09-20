import { useI18n } from "@voya/i18n/use-i18n";
import { SegmentedControl, SegmentedControlItem } from "@voya/ui/components/segmented-control";

import { ModeInfo } from "./mode-info";
import { useTrafficMode } from "@voya/features/routing/use-traffic-mode";

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
  const { disabled, disabledReason, mode, selectMode } = useTrafficMode();

  return (
    <div className="flex items-center gap-2">
      <span className="inline-flex items-center gap-1 text-sm text-muted-foreground">
        <span id="routing-traffic-mode-label">{t("panes.routing.trafficMode")}</span>
        <ModeInfo
          hint={t("panes.routing.trafficModeHint")}
          label={t("panes.routing.trafficModeInfo")}
        />
      </span>
      <SegmentedControl
        aria-labelledby="routing-traffic-mode-label"
        title={disabled && disabledReason ? t(disabledReason) : undefined}
      >
        {MODES.map(({ value, labelKey }) => (
          <SegmentedControlItem
            disabled={disabled}
            key={value}
            onClick={() => selectMode(value)}
            pressed={mode === value}
          >
            {t(labelKey)}
          </SegmentedControlItem>
        ))}
      </SegmentedControl>
    </div>
  );
}
