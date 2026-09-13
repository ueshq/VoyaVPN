import { Globe } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";

import { PageSurface } from "@/components/app-shell/page-section";
import { useTrafficMode } from "@/features/home/use-traffic-mode";

/**
 * Global mode sends all captured traffic through the proxy ahead of every rule
 * (`clash_mode: Global`), so nothing on this page applies while it is on.
 */
export function TrafficModeBanner() {
  const { t } = useI18n();
  const { disabled, mode, selectMode } = useTrafficMode();
  if (mode !== "global") {
    return null;
  }

  return (
    <PageSurface
      className="flex flex-wrap items-center gap-x-3 gap-y-2 bg-accent-blue-bg px-4 py-2.5"
      role="status"
    >
      <Globe aria-hidden="true" className="size-4 shrink-0 text-accent-blue" />
      <p className="min-w-0 flex-1 text-sm">{t("panes.routing.globalModeBanner")}</p>
      <Button
        disabled={disabled}
        onClick={() => selectMode("rule")}
        size="sm"
        type="button"
        variant="outline"
      >
        {t("panes.routing.globalModeSwitchBack")}
      </Button>
    </PageSurface>
  );
}
