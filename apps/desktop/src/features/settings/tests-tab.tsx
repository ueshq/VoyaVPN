import { NumberField, TextField, SettingsGroup } from "./settings-form";
import { useI18n } from "@voya/i18n/use-i18n";

import type { AppSettingsController } from "./use-app-settings";

export function TestsTab({ controller }: { controller: AppSettingsController }) {
  const { t } = useI18n();
  const { settings, error, update, working } = controller;

  if (!settings) {
    return <p className="text-xs text-muted-foreground">{working ? t("options.loading") : error}</p>;
  }

  const patchTests = (patch: Partial<typeof settings.speedTest>) =>
    update((current) => ({
      ...current,
      speedTest: { ...current.speedTest, ...patch },
    }));

  return (
    <div className="grid gap-4">
      <SettingsGroup>
        <NumberField id="rt-speedtest-timeout" label={t("settings.tests.timeout")} onChange={(timeoutSeconds) => patchTests({ timeoutSeconds: timeoutSeconds ?? 0 })} value={settings.speedTest.timeoutSeconds} />
        <NumberField id="rt-speedtest-concurrency" label={t("settings.tests.proxyDelayConcurrency")} onChange={(proxyDelayConcurrency) => patchTests({ proxyDelayConcurrency: proxyDelayConcurrency ?? 0 })} value={settings.speedTest.proxyDelayConcurrency} />
        <TextField id="rt-speedtest-ping-url" label={t("settings.tests.pingUrl")} onChange={(latencyUrl) => patchTests({ latencyUrl })} value={settings.speedTest.latencyUrl} />
        <TextField id="rt-speedtest-ipapi-url" label={t("settings.tests.ipApiUrl")} onChange={(ipLookupUrl) => patchTests({ ipLookupUrl })} value={settings.speedTest.ipLookupUrl} />
        <NumberField id="rt-speedtest-page-size" label={t("settings.fields.speedTestPageSize")} onChange={(pageSize) => patchTests({ pageSize })} value={settings.speedTest.pageSize} />
        <NumberField id="rt-speedtest-delay-interval" label={t("settings.fields.speedTestDelayInterval")} onChange={(delayIntervalSeconds) => patchTests({ delayIntervalSeconds })} value={settings.speedTest.delayIntervalSeconds} />
      </SettingsGroup>
    </div>
  );
}
