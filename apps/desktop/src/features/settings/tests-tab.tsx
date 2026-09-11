import { NumberField, TextField, SettingsGroup } from "./settings-form";
import { useI18n } from "@voya/i18n/use-i18n";

import type { AppSettingsFormController } from "./use-app-settings";

export function TestsTab({ controller }: { controller: AppSettingsFormController }) {
  const { t } = useI18n();
  const { settings, update } = controller;

  const patchTests = (patch: Partial<typeof settings.speedTest>) =>
    update((current) => ({
      ...current,
      speedTest: { ...current.speedTest, ...patch },
    }));

  return (
    <div className="grid gap-4">
      <SettingsGroup title={t("settings.sections.testExecution")}>
        <NumberField field="speedTest.timeoutSeconds" id="rt-speedtest-timeout" label={t("settings.tests.timeout")} onChange={(timeoutSeconds) => patchTests({ timeoutSeconds: timeoutSeconds ?? 0 })} value={settings.speedTest.timeoutSeconds} />
        <NumberField nullable field="speedTest.pageSize" id="rt-speedtest-page-size" label={t("settings.fields.speedTestPageSize")} onChange={(pageSize) => patchTests({ pageSize })} value={settings.speedTest.pageSize} />
        <NumberField nullable field="speedTest.delayIntervalSeconds" id="rt-speedtest-delay-interval" label={t("settings.fields.speedTestDelayInterval")} onChange={(delayIntervalSeconds) => patchTests({ delayIntervalSeconds })} value={settings.speedTest.delayIntervalSeconds} />
      </SettingsGroup>

      <SettingsGroup title={t("settings.sections.testAddresses")}>
        <TextField field="speedTest.latencyUrl" id="rt-speedtest-ping-url" label={t("settings.tests.pingUrl")} onChange={(latencyUrl) => patchTests({ latencyUrl })} value={settings.speedTest.latencyUrl} />
        <TextField description={t("settings.tests.ipApiUrlHint")} field="speedTest.ipLookupUrl" id="rt-speedtest-ipapi-url" label={t("settings.tests.ipApiUrl")} onChange={(ipLookupUrl) => patchTests({ ipLookupUrl })} value={settings.speedTest.ipLookupUrl} />
      </SettingsGroup>
    </div>
  );
}
