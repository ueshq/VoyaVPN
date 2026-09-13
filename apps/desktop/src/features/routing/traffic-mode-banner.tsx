import { Globe } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";
import { getErrorMessage } from "@voya/utils/error";

import { PageSurface } from "@/components/app-shell/page-section";

import { useSavedTrafficMode } from "./use-traffic-mode";

/**
 * Global mode sends all captured traffic through the proxy ahead of every rule
 * (`clash_mode: Global`), so nothing on this page applies while it is on. The
 * switcher in the page title changes the mode; this says why the rules are
 * locked, or that the mode could not be read.
 */
export function TrafficModeBanner() {
  const { t } = useI18n();
  const { error, mode, retry } = useSavedTrafficMode();
  if (error) {
    return (
      <PageSurface className="flex flex-wrap items-center gap-x-3 gap-y-2 px-4 py-2.5" role="alert">
        <p className="min-w-0 flex-1 text-sm text-muted-foreground">{getErrorMessage(error)}</p>
        <Button onClick={retry} size="sm" type="button" variant="ghost">
          {t("actions.retry")}
        </Button>
      </PageSurface>
    );
  }
  if (mode !== "global") {
    return null;
  }

  return (
    <PageSurface className="flex items-center gap-3 bg-accent-blue-bg px-4 py-2.5" role="status">
      <Globe aria-hidden="true" className="size-4 shrink-0 text-accent-blue" />
      <p className="min-w-0 flex-1 text-sm">{t("panes.routing.globalModeBanner")}</p>
    </PageSurface>
  );
}
