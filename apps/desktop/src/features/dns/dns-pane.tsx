import { Badge } from "@voya/ui/components/badge";
import { useI18n } from "@voya/i18n/use-i18n";

import { SimpleDnsForm } from "./simple-dns-form";
import type { useDnsSettings } from "./use-dns-settings";

export function DnsPane({ controller }: { controller: ReturnType<typeof useDnsSettings> }) {
  const { t } = useI18n();
  const { fieldErrors, form, issueCount, updateSimple } = controller;
  return (
    <section aria-label={t("panes.dns.title")} className="grid gap-4">
      <div className="flex items-center gap-2">
        <Badge variant="outline">{form?.fakeIp ? t("panes.dns.fakeIp") : t("panes.dns.standard")}</Badge>
        {issueCount ? <Badge variant="destructive">{t("panes.dns.errorCount", { count: issueCount })}</Badge> : null}
      </div>
      {form ? <SimpleDnsForm errors={fieldErrors} settings={form} updateSimple={updateSimple} /> : <p className="text-sm text-muted-foreground">{t("panes.dns.loading")}</p>}
    </section>
  );
}
