import { RefreshCw, Save, TriangleAlert } from "lucide-react";

import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { cn } from "@voya/ui/lib/utils";
import { useI18n } from "@voya/i18n/use-i18n";

import { SimpleDnsForm } from "./simple-dns-form";
import { useDnsSettings } from "./use-dns-settings";

/**
 * DNS settings pane embedded in the Settings surface. It keeps its own
 * Reload/Save lifecycle (dedicated commands with server-side validation)
 * instead of joining the surface's save-all draft, mirroring the Updates tab
 * precedent. The hosting `TabsContent` provides scroll and padding.
 */
export function DnsPane() {
  const { t } = useI18n();
  const {
    dnsQuery,
    fieldErrors,
    form,
    handleReload,
    handleSave,
    isDirty,
    issueCount,
    operationError,
    updateSimple,
  } = useDnsSettings();

  return (
    <section aria-label={t("panes.dns.title")} className="mx-auto grid w-full max-w-3xl gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <h3 className="text-sm font-semibold">{t("panes.dns.title")}</h3>
        <Badge variant="outline">{form?.fakeIp ? t("panes.dns.fakeIp") : t("panes.dns.standard")}</Badge>
        {issueCount ? (
          <Badge variant="destructive">
            <TriangleAlert className="size-3.5" aria-hidden="true" />
            {t("panes.dns.errorCount", { count: issueCount })}
          </Badge>
        ) : null}
        <div className="ms-auto flex items-center gap-2">
          <Button disabled={dnsQuery.isFetching} onClick={() => void handleReload()} size="sm" type="button" variant="outline">
            <RefreshCw className={cn("size-4", dnsQuery.isFetching && "animate-spin")} aria-hidden="true" />
            {t("actions.reload")}
          </Button>
          <Button disabled={!form || !isDirty} onClick={() => void handleSave()} size="sm" type="button">
            <Save className="size-4" aria-hidden="true" />
            {t("actions.save")}
          </Button>
        </div>
      </div>

      {operationError ? (
        <Alert className="py-2" variant="destructive">
          <TriangleAlert aria-hidden="true" />
          <AlertDescription>{operationError}</AlertDescription>
        </Alert>
      ) : null}

      {form ? (
        <SimpleDnsForm errors={fieldErrors} settings={form} updateSimple={updateSimple} />
      ) : (
        <div className="text-sm text-muted-foreground">{t("panes.dns.loading")}</div>
      )}
    </section>
  );
}
