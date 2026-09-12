import { Badge } from "@voya/ui/components/badge";
import { Disclosure } from "@voya/ui/components/disclosure";
import {
  CheckboxField,
  SelectField,
  TextAreaField,
  TextField,
} from "@voya/ui/components/form-fields";
import { useI18n } from "@voya/i18n/use-i18n";
import { SettingsGroup } from "@/features/settings/settings-form";
import type { DnsSettings } from "@/ipc/bindings";

import { DNS_STRATEGIES } from "./dns-form-schema";
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

function SimpleDnsForm({
  errors,
  settings,
  updateSimple,
}: {
  errors: Record<string, string>;
  settings: DnsSettings;
  updateSimple: (patch: Partial<DnsSettings>) => void;
}) {
  const { t } = useI18n();
  const strategyLabels = {
    "": t("panes.routing.defaultValue"), AsIs: t("panes.dns.strategyAutomatic"), UseIP: t("panes.dns.strategyAutomatic"),
    UseIPv4: t("panes.dns.strategyIpv4"), UseIPv6: t("panes.dns.strategyIpv6"), ForceIPv4: t("panes.dns.strategyOnlyIpv4"), ForceIPv6: t("panes.dns.strategyOnlyIpv6"),
  };
  const strategies = DNS_STRATEGIES.map((value) => ({ value, label: strategyLabels[value] }));
  return (
    <>
      <SettingsGroup title={t("settings.sections.dnsServers")}>
        <TextField
          commitOnBlur
          error={errors.direct}
          label={t("panes.dns.directDns")}
          layout="row"
          onChange={(direct) => updateSimple({ direct })}
          value={settings.direct ?? ""}
        />
        <TextField
          commitOnBlur
          error={errors.remote}
          label={t("panes.dns.remoteDns")}
          layout="row"
          onChange={(remote) => updateSimple({ remote })}
          value={settings.remote ?? ""}
        />
        <TextField
          commitOnBlur
          error={errors.bootstrap}
          label={t("panes.dns.bootstrapDns")}
          layout="row"
          onChange={(bootstrap) => updateSimple({ bootstrap })}
          value={settings.bootstrap ?? ""}
        />
        <SelectField
          options={strategies}
          description={t("panes.dns.strategyHint")}
          label={t("panes.dns.directStrategy")}
          layout="row"
          onChange={(value) => updateSimple({ directStrategy: value || null })}
          value={settings.directStrategy ?? ""}
        />
        <SelectField
          options={strategies}
          description={t("panes.dns.strategyHint")}
          label={t("panes.dns.proxyStrategy")}
          layout="row"
          onChange={(value) => updateSimple({ proxyStrategy: value || null })}
          value={settings.proxyStrategy ?? ""}
        />
      </SettingsGroup>
      <SettingsGroup title={t("settings.sections.dnsBehavior")}>
        <div className="grid gap-3 @min-[42rem]:grid-cols-2">
          <CheckboxField
            checked={Boolean(settings.addCommonHosts)}
            label={t("panes.dns.commonHosts")}
            onChange={(addCommonHosts) => updateSimple({ addCommonHosts })}
          />
          <CheckboxField
            checked={Boolean(settings.blockBindingQuery)}
            label={t("panes.dns.blockBindingQuery")}
            onChange={(blockBindingQuery) =>
              updateSimple({ blockBindingQuery })
            }
          />
          <CheckboxField
            checked={Boolean(settings.fakeIp)}
            label={t("panes.dns.fakeIp")}
            onChange={(fakeIp) => updateSimple({ fakeIp })}
          />
          <CheckboxField
            checked={Boolean(settings.globalFakeIp)}
            disabled={!settings.fakeIp}
            label={t("panes.dns.globalFakeIp")}
              description={t("panes.dns.globalFakeIpHint")}
            onChange={(globalFakeIp) => updateSimple({ globalFakeIp })}
          />
        </div>
      </SettingsGroup>
      <Disclosure
        className="border-0 [&>div]:border-0"
        title={t("common.advanced")}
        invalid={!!errors.hosts || !!errors.directExpectedIps}
      >
        <SettingsGroup title={t("settings.sections.dnsHosts")}>
          <TextAreaField
            commitOnBlur
            error={errors.hosts}
            label={t("panes.dns.hosts")}
            layout="row"
            onChange={(hosts) => updateSimple({ hosts })}
            value={settings.hosts ?? ""}
          />
          <TextAreaField
            commitOnBlur
            error={errors.directExpectedIps}
            description={t("panes.dns.expectedHint")}
            label={t("panes.dns.expectedIps")}
            layout="row"
            onChange={(directExpectedIps) =>
              updateSimple({ directExpectedIps })
            }
            value={settings.directExpectedIps ?? ""}
          />
        </SettingsGroup>
      </Disclosure>
    </>
  );
}
