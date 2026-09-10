import { SettingsCheckbox, NumberField, SelectField, TextField, SettingsCheckboxGroup, SettingsGroup, SettingsRow } from "./settings-form";
import { Separator } from "@voya/ui/components/separator";
import { useI18n } from "@voya/i18n/use-i18n";

import type { AppSettingsController } from "./use-app-settings";

export function CoreTab({ controller }: { controller: AppSettingsController }) {
  const { t } = useI18n();
  const { settings, error, update, working } = controller;

  if (!settings) {
    return <p className="text-xs text-muted-foreground">{working ? t("options.loading") : error}</p>;
  }

  const patchCore = (patch: Partial<typeof settings.core>) =>
    update((current) => ({
      ...current,
      core: { ...current.core, ...patch },
    }));
  const patchMux = (patch: Partial<typeof settings.multiplexing>) =>
    update((current) => ({
      ...current,
      multiplexing: { ...current.multiplexing, ...patch },
    }));
  const patchHysteria = (patch: Partial<typeof settings.hysteria>) =>
    update((current) => ({
      ...current,
      hysteria: { ...current.hysteria, ...patch },
    }));

  return (
    <div className="grid gap-4">
      <SettingsGroup>
        <SettingsCheckboxGroup id="rt-core-basics" label={t("settings.core.title")}>
          <SettingsCheckbox checked={settings.core.logEnabled} label={t("settings.core.logEnabled")} onCheckedChange={(logEnabled) => patchCore({ logEnabled: logEnabled === true })} />
          <SettingsCheckbox checked={settings.core.defaultAllowInsecure} label={t("settings.core.allowInsecure")} onCheckedChange={(defaultAllowInsecure) => patchCore({ defaultAllowInsecure: defaultAllowInsecure === true })} />
          <SettingsCheckbox checked={settings.core.muxEnabled} label={t("settings.core.muxEnabled")} onCheckedChange={(muxEnabled) => patchCore({ muxEnabled: muxEnabled === true })} />
          <SettingsCheckbox checked={settings.core.fragmentEnabled} label={t("settings.core.fragmentEnabled")} onCheckedChange={(fragmentEnabled) => patchCore({ fragmentEnabled: fragmentEnabled === true })} />
          <SettingsCheckbox checked={settings.core.cacheFileEnabled} label={t("settings.core.cacheFileEnabled")} onCheckedChange={(cacheFileEnabled) => patchCore({ cacheFileEnabled: cacheFileEnabled === true })} />
        </SettingsCheckboxGroup>
        <SelectField id="rt-loglevel" label={t("settings.core.logLevel")} onChange={(logLevel) => patchCore({ logLevel })} options={["none", "trace", "debug", "info", "warn", "warning", "error"]} value={settings.core.logLevel} />
        <TextField id="rt-fingerprint" label={t("settings.core.fingerprint")} onChange={(defaultFingerprint) => patchCore({ defaultFingerprint })} value={settings.core.defaultFingerprint} />
        <TextField id="rt-user-agent" label={t("settings.core.userAgent")} onChange={(defaultUserAgent) => patchCore({ defaultUserAgent })} value={settings.core.defaultUserAgent} />
        <TextField id="rt-send-through" label={t("settings.core.sendThrough")} onChange={(sendThrough) => patchCore({ sendThrough: nullableText(sendThrough) })} value={settings.core.sendThrough ?? ""} />
        <TextField id="rt-bind-interface" label={t("settings.core.bindInterface")} onChange={(bindInterface) => patchCore({ bindInterface: nullableText(bindInterface) })} value={settings.core.bindInterface ?? ""} />
      </SettingsGroup>

      <Separator />

      <SettingsGroup>
        <TextField id="rt-mux-sbox-protocol" label={t("settings.core.muxProtocol")} onChange={(protocol) => patchMux({ protocol })} value={settings.multiplexing.protocol} />
        <NumberField id="rt-mux-sbox-max-connections" label={t("settings.fields.muxMaxConnections")} onChange={(maxConnections) => patchMux({ maxConnections: maxConnections ?? 0 })} value={settings.multiplexing.maxConnections} />
        <SettingsRow>
          <SettingsCheckbox checked={settings.multiplexing.padding ?? false} label={t("settings.fields.muxPadding")} onCheckedChange={(padding) => patchMux({ padding: padding === true })} />
        </SettingsRow>
      </SettingsGroup>

      <Separator />

      <SettingsGroup>
        <SettingsRow>
          <p className="text-xs font-medium text-muted-foreground">{t("settings.core.hysteriaBandwidth")}</p>
        </SettingsRow>
        <NumberField id="rt-hysteria-up" label={t("settings.fields.hysteriaUpMbps")} onChange={(uploadMbps) => patchHysteria({ uploadMbps: uploadMbps ?? 0 })} value={settings.hysteria.uploadMbps} />
        <NumberField id="rt-hysteria-down" label={t("settings.fields.hysteriaDownMbps")} onChange={(downloadMbps) => patchHysteria({ downloadMbps: downloadMbps ?? 0 })} value={settings.hysteria.downloadMbps} />
        <NumberField id="rt-hysteria-hop-interval" label={t("settings.core.hysteriaHopInterval")} onChange={(hopIntervalSeconds) => patchHysteria({ hopIntervalSeconds: hopIntervalSeconds ?? 5 })} value={settings.hysteria.hopIntervalSeconds} />
      </SettingsGroup>
    </div>
  );
}

function nullableText(value: string): string | null {
  return value.trim() ? value : null;
}
