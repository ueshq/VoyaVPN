import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { Disclosure } from "@voya/ui/components/disclosure";
import {
  SelectField,
  SwitchField,
  TextAreaField,
  TextField,
} from "@voya/ui/components/form-fields";
import { cn } from "@voya/ui/lib/utils";
import type { TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { SettingsGroup } from "@/features/settings/settings-form";
import type { DnsSettings } from "@/ipc/bindings";

import { DNS_STRATEGIES } from "./dns-form-schema";
import type { useDnsSettings } from "./use-dns-settings";

// "AsIs" and "UseIP" generate no strategy at all, exactly like the default, so
// the list offers the default once and shows either stored value as it.
const DEFAULT_LIKE_STRATEGIES: readonly string[] = ["AsIs", "UseIP"];
const STRATEGY_OPTIONS = DNS_STRATEGIES.filter(
  (value) => !DEFAULT_LIKE_STRATEGIES.includes(value),
);

function shownStrategy(value: string | null) {
  return value && !DEFAULT_LIKE_STRATEGIES.includes(value) ? value : "";
}

type DnsPreset = { labelKey: TranslationKey; value: string };

const DIRECT_PRESETS = [
  { labelKey: "panes.dns.presets.dnspod", value: "119.29.29.29" },
  { labelKey: "panes.dns.presets.aliyun", value: "223.5.5.5" },
] as const satisfies readonly DnsPreset[];

const REMOTE_PRESETS = [
  { labelKey: "panes.dns.presets.cloudflare", value: "https://cloudflare-dns.com/dns-query" },
  { labelKey: "panes.dns.presets.google", value: "https://dns.google/dns-query" },
] as const satisfies readonly DnsPreset[];

export function DnsPane({ controller }: { controller: ReturnType<typeof useDnsSettings> }) {
  const { t } = useI18n();
  const { fieldErrors, form, issueCount, updateSimple } = controller;
  return (
    <section aria-label={t("panes.dns.title")} className="grid gap-4">
      {form ? (
        <SimpleDnsForm
          errors={fieldErrors}
          issueCount={issueCount}
          settings={form}
          updateSimple={updateSimple}
        />
      ) : (
        <p className="text-sm text-muted-foreground">{t("panes.dns.loading")}</p>
      )}
    </section>
  );
}

function SimpleDnsForm({
  errors,
  issueCount,
  settings,
  updateSimple,
}: {
  errors: Record<string, string>;
  issueCount: number;
  settings: DnsSettings;
  updateSimple: (patch: Partial<DnsSettings>) => void;
}) {
  const { t } = useI18n();
  const strategyLabels: Record<string, string> = {
    "": t("panes.routing.defaultValue"),
    UseIPv4: t("panes.dns.strategyIpv4"), UseIPv6: t("panes.dns.strategyIpv6"), ForceIPv4: t("panes.dns.strategyOnlyIpv4"), ForceIPv6: t("panes.dns.strategyOnlyIpv6"),
  };
  const strategies = STRATEGY_OPTIONS.map((value) => ({ value, label: strategyLabels[value] ?? value }));
  return (
    <>
      {/* The FakeIP switch below already shows the mode, so only errors get a badge. */}
      <SettingsGroup
        actions={
          issueCount ? (
            <Badge variant="destructive">{t("panes.dns.errorCount", { count: issueCount })}</Badge>
          ) : undefined
        }
        title={t("settings.sections.dnsServers")}
      >
        <TextField
          addon={
            <DnsPresets
              current={settings.direct}
              label={t("panes.dns.presetsDirect")}
              onPick={(direct) => updateSimple({ direct })}
              presets={DIRECT_PRESETS}
            />
          }
          commitOnBlur
          error={errors.direct}
          label={t("panes.dns.directDns")}
          layout="row"
          onChange={(direct) => updateSimple({ direct })}
          value={settings.direct ?? ""}
        />
        <TextField
          addon={
            <DnsPresets
              current={settings.remote}
              label={t("panes.dns.presetsRemote")}
              onPick={(remote) => updateSimple({ remote })}
              presets={REMOTE_PRESETS}
            />
          }
          commitOnBlur
          error={errors.remote}
          label={t("panes.dns.remoteDns")}
          layout="row"
          onChange={(remote) => updateSimple({ remote })}
          value={settings.remote ?? ""}
        />
        <TextField
          commitOnBlur
          description={t("panes.dns.bootstrapHint")}
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
          value={shownStrategy(settings.directStrategy)}
        />
        <SelectField
          options={strategies}
          description={t("panes.dns.strategyHint")}
          label={t("panes.dns.proxyStrategy")}
          layout="row"
          onChange={(value) => updateSimple({ proxyStrategy: value || null })}
          value={shownStrategy(settings.proxyStrategy)}
        />
      </SettingsGroup>
      <SettingsGroup title={t("settings.sections.dnsBehavior")}>
        <SwitchField
          checked={Boolean(settings.addCommonHosts)}
          description={t("panes.dns.commonHostsHint")}
          label={t("panes.dns.commonHosts")}
          onChange={(addCommonHosts) => updateSimple({ addCommonHosts })}
        />
        <SwitchField
          checked={Boolean(settings.blockBindingQuery)}
          description={t("panes.dns.blockBindingQueryHint")}
          label={t("panes.dns.blockBindingQuery")}
          onChange={(blockBindingQuery) =>
            updateSimple({ blockBindingQuery })
          }
        />
        <SwitchField
          checked={Boolean(settings.fakeIp)}
          description={t("panes.dns.fakeIpHint")}
          label={t("panes.dns.fakeIp")}
          onChange={(fakeIp) => updateSimple({ fakeIp })}
        />
        <SwitchField
          checked={Boolean(settings.fakeIp && settings.globalFakeIp)}
          disabled={!settings.fakeIp}
          label={t("panes.dns.globalFakeIp")}
          description={t("panes.dns.globalFakeIpHint")}
          onChange={(globalFakeIp) => updateSimple({ globalFakeIp })}
        />
      </SettingsGroup>
      <Disclosure
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

/** One-click servers right under their DNS field; the field itself stays free text. */
function DnsPresets({
  current,
  label,
  onPick,
  presets,
}: {
  current: string | null;
  label: string;
  onPick: (value: string) => void;
  presets: readonly DnsPreset[];
}) {
  const { t } = useI18n();
  return (
    <div aria-label={label} className="flex flex-wrap gap-2" role="group">
      {presets.map((preset) => {
        const selected = current?.trim() === preset.value;
        return (
          <Button
            aria-pressed={selected}
            className={cn(
              selected &&
                "border-primary bg-accent-blue-light text-brand hover:bg-accent-blue-light hover:text-brand",
            )}
            key={preset.value}
            onClick={() => onPick(preset.value)}
            size="xs"
            type="button"
            variant="outline"
          >
            {t(preset.labelKey)}
          </Button>
        );
      })}
    </div>
  );
}
