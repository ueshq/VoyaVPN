import type { DnsSettings } from "@/ipc/bindings";
import { useI18n } from "@voya/i18n/use-i18n";
import { CheckboxField, SelectField, TextAreaField, TextField } from "@voya/ui/components/form-fields";
import { SettingsGroup } from "@/features/settings/settings-form";

import { DNS_STRATEGIES } from "./dns-constants";

export function SimpleDnsForm({ errors, settings, updateSimple }: {
  errors: Record<string, string>;
  settings: DnsSettings;
  updateSimple: (patch: Partial<DnsSettings>) => void;
}) {
  const { t } = useI18n();
  const strategies = DNS_STRATEGIES.map((value) => ({ value, label: value || t("panes.routing.defaultValue") }));
  return (
    <>
      <SettingsGroup title={t("settings.sections.dnsBehavior")}>
        <div className="grid gap-3 @min-[42rem]:grid-cols-2">
          <CheckboxField checked={Boolean(settings.addCommonHosts)} label={t("panes.dns.commonHosts")} onChange={(addCommonHosts) => updateSimple({ addCommonHosts })} />
          <CheckboxField checked={Boolean(settings.blockBindingQuery)} label={t("panes.dns.blockBindingQuery")} onChange={(blockBindingQuery) => updateSimple({ blockBindingQuery })} />
          <CheckboxField checked={Boolean(settings.fakeIp)} label={t("panes.dns.fakeIp")} onChange={(fakeIp) => updateSimple({ fakeIp })} />
          <CheckboxField checked={Boolean(settings.globalFakeIp)} disabled={!settings.fakeIp} label={t("panes.dns.globalFakeIp")} onChange={(globalFakeIp) => updateSimple({ globalFakeIp })} />
        </div>
      </SettingsGroup>
      <SettingsGroup title={t("settings.sections.dnsServers")}>
        <TextField commitOnBlur error={errors.direct} label={t("panes.dns.directDns")} layout="row" onChange={(direct) => updateSimple({ direct })} value={settings.direct ?? ""} />
        <TextField commitOnBlur error={errors.remote} label={t("panes.dns.remoteDns")} layout="row" onChange={(remote) => updateSimple({ remote })} value={settings.remote ?? ""} />
        <TextField commitOnBlur error={errors.bootstrap} label={t("panes.dns.bootstrapDns")} layout="row" onChange={(bootstrap) => updateSimple({ bootstrap })} value={settings.bootstrap ?? ""} />
        <SelectField options={strategies} label={t("panes.dns.directStrategy")} layout="row" onChange={(value) => updateSimple({ directStrategy: value || null })} value={settings.directStrategy ?? ""} />
        <SelectField options={strategies} label={t("panes.dns.proxyStrategy")} layout="row" onChange={(value) => updateSimple({ proxyStrategy: value || null })} value={settings.proxyStrategy ?? ""} />
      </SettingsGroup>
      <SettingsGroup title={t("settings.sections.dnsHosts")}>
        <TextAreaField commitOnBlur error={errors.hosts} label={t("panes.dns.hosts")} layout="row" onChange={(hosts) => updateSimple({ hosts })} value={settings.hosts ?? ""} />
        <TextAreaField commitOnBlur error={errors.directExpectedIps} label={t("panes.dns.expectedIps")} layout="row" onChange={(directExpectedIps) => updateSimple({ directExpectedIps })} value={settings.directExpectedIps ?? ""} />
      </SettingsGroup>
    </>
  );
}
