import { ShieldCheck } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@voya/ui/components/card";
import type { DnsSettings } from "@/ipc/bindings";
import { useI18n } from "@voya/i18n/use-i18n";

import { CheckboxField, SelectField, TextAreaField, TextField } from "./dns-form-fields";

export function SimpleDnsForm({
  errors,
  settings,
  updateSimple,
}: {
  /** Field path → already-translated message (see lib/zod-errors.ts). */
  errors: Record<string, string>;
  settings: DnsSettings;
  updateSimple: (patch: Partial<DnsSettings>) => void;
}) {
  const { t } = useI18n();

  return (
    <Card className="gap-3 rounded-xl bg-surface-raised p-3 shadow-raised">
      <CardHeader className="p-0">
        <CardTitle className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground">
          <ShieldCheck className="size-4 text-muted-foreground" aria-hidden="true" />
          {t("panes.dns.simpleTitle")}
        </CardTitle>
      </CardHeader>
      <CardContent className="grid gap-4 p-0">
        <div className="grid gap-2">
          <CheckboxField
            checked={Boolean(settings.addCommonHosts)}
            label={t("panes.dns.commonHosts")}
            onChange={(value) => updateSimple({ addCommonHosts: value })}
          />
          <CheckboxField
            checked={Boolean(settings.blockBindingQuery)}
            label={t("panes.dns.blockBindingQuery")}
            onChange={(value) => updateSimple({ blockBindingQuery: value })}
          />
          <CheckboxField
            checked={Boolean(settings.fakeIp)}
            label={t("panes.dns.fakeIp")}
            onChange={(value) => updateSimple({ fakeIp: value })}
          />
          <CheckboxField
            checked={Boolean(settings.globalFakeIp)}
            disabled={!settings.fakeIp}
            label={t("panes.dns.globalFakeIp")}
            onChange={(value) => updateSimple({ globalFakeIp: value })}
          />
        </div>

        <TextField
          error={errors.direct}
          label={t("panes.dns.directDns")}
          onChange={(value) => updateSimple({ direct: value })}
          value={settings.direct ?? ""}
        />
        <TextField
          error={errors.remote}
          label={t("panes.dns.remoteDns")}
          onChange={(value) => updateSimple({ remote: value })}
          value={settings.remote ?? ""}
        />
        <TextField
          error={errors.bootstrap}
          label={t("panes.dns.bootstrapDns")}
          onChange={(value) => updateSimple({ bootstrap: value })}
          value={settings.bootstrap ?? ""}
        />

        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-1">
          <SelectField
            label={t("panes.dns.directStrategy")}
            onChange={(value) => updateSimple({ directStrategy: value || null })}
            value={settings.directStrategy ?? ""}
          />
          <SelectField
            label={t("panes.dns.proxyStrategy")}
            onChange={(value) => updateSimple({ proxyStrategy: value || null })}
            value={settings.proxyStrategy ?? ""}
          />
        </div>

        <TextAreaField
          error={errors.hosts}
          label={t("panes.dns.hosts")}
          onChange={(value) => updateSimple({ hosts: value })}
          value={settings.hosts ?? ""}
        />
        <TextAreaField
          error={errors.directExpectedIps}
          label={t("panes.dns.expectedIps")}
          onChange={(value) => updateSimple({ directExpectedIps: value })}
          value={settings.directExpectedIps ?? ""}
        />
      </CardContent>
    </Card>
  );
}
