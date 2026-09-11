import { Disclosure } from "@voya/ui/components/disclosure";
import { nullableText } from "./settings-values";
import {
  SettingsCheckbox,
  NumberField,
  SelectField,
  TextField,
  SettingsGroup,
  SettingsRow,
} from "./settings-form";
import { useI18n } from "@voya/i18n/use-i18n";

import type { AppSettingsFormController } from "./use-app-settings";

export function CoreTab({
  controller,
}: {
  controller: AppSettingsFormController;
}) {
  const { t } = useI18n();
  const { settings, update } = controller;

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
      <SettingsGroup title={t("settings.sections.coreBasics")}>
        <div className="grid gap-3 @min-[42rem]:grid-cols-2">
          <SettingsCheckbox
            field="core.logEnabled"
            checked={settings.core.logEnabled}
            label={t("settings.core.logEnabled")}
            onCheckedChange={(logEnabled) =>
              patchCore({ logEnabled: logEnabled === true })
            }
          />
          <SettingsCheckbox
            field="core.defaultAllowInsecure"
            checked={settings.core.defaultAllowInsecure}
            label={t("settings.core.allowInsecure")}
            onCheckedChange={(defaultAllowInsecure) =>
              patchCore({ defaultAllowInsecure: defaultAllowInsecure === true })
            }
          />
          <SettingsCheckbox
            field="core.muxEnabled"
            checked={settings.core.muxEnabled}
            label={t("settings.core.muxEnabled")}
            onCheckedChange={(muxEnabled) =>
              patchCore({ muxEnabled: muxEnabled === true })
            }
          />
          <SettingsCheckbox
            field="core.fragmentEnabled"
            checked={settings.core.fragmentEnabled}
            label={t("settings.core.fragmentEnabled")}
            onCheckedChange={(fragmentEnabled) =>
              patchCore({ fragmentEnabled: fragmentEnabled === true })
            }
          />
          <SettingsCheckbox
            field="core.cacheFileEnabled"
            checked={settings.core.cacheFileEnabled}
            label={t("settings.core.cacheFileEnabled")}
            onCheckedChange={(cacheFileEnabled) =>
              patchCore({ cacheFileEnabled: cacheFileEnabled === true })
            }
          />
        </div>
        <SelectField
          field="core.logLevel"
          id="rt-loglevel"
          label={t("settings.core.logLevel")}
          onChange={(logLevel) => patchCore({ logLevel })}
          options={[
            "none",
            "trace",
            "debug",
            "info",
            "warn",
            "warning",
            "error",
          ]}
          value={settings.core.logLevel}
        />
      </SettingsGroup>

      <Disclosure
        title={t("common.advanced")}
        invalid={Object.keys(controller.fieldErrors).length > 0}
      >
        <SettingsGroup title={t("settings.sections.outbound")}>
          <TextField
            field="core.defaultFingerprint"
            id="rt-fingerprint"
            label={t("settings.core.fingerprint")}
            onChange={(defaultFingerprint) => patchCore({ defaultFingerprint })}
            value={settings.core.defaultFingerprint}
          />
          <TextField
            field="core.defaultUserAgent"
            id="rt-user-agent"
            label={t("settings.core.userAgent")}
            onChange={(defaultUserAgent) => patchCore({ defaultUserAgent })}
            value={settings.core.defaultUserAgent}
          />
          <TextField
            field="core.sendThrough"
            id="rt-send-through"
            label={t("settings.core.sendThrough")}
            onChange={(sendThrough) =>
              patchCore({ sendThrough: nullableText(sendThrough) })
            }
            value={settings.core.sendThrough ?? ""}
          />
          <TextField
            field="core.bindInterface"
            id="rt-bind-interface"
            label={t("settings.core.bindInterface")}
            onChange={(bindInterface) =>
              patchCore({ bindInterface: nullableText(bindInterface) })
            }
            value={settings.core.bindInterface ?? ""}
          />
        </SettingsGroup>

        <SettingsGroup title={t("settings.sections.multiplexing")}>
          <TextField
            field="multiplexing.protocol"
            id="rt-mux-sbox-protocol"
            label={t("settings.core.muxProtocol")}
            onChange={(protocol) => patchMux({ protocol })}
            value={settings.multiplexing.protocol}
          />
          <NumberField
            field="multiplexing.maxConnections"
            id="rt-mux-sbox-max-connections"
            label={t("settings.fields.muxMaxConnections")}
            onChange={(maxConnections) =>
              patchMux({ maxConnections: maxConnections ?? 0 })
            }
            value={settings.multiplexing.maxConnections}
          />
          <SettingsRow>
            <SettingsCheckbox
              field="multiplexing.padding"
              checked={settings.multiplexing.padding ?? false}
              label={t("settings.fields.muxPadding")}
              onCheckedChange={(padding) =>
                patchMux({ padding: padding === true })
              }
            />
          </SettingsRow>
        </SettingsGroup>

        <SettingsGroup title={t("settings.core.hysteriaBandwidth")}>
          <NumberField
            field="hysteria.uploadMbps"
            id="rt-hysteria-up"
            label={t("settings.fields.hysteriaUpMbps")}
            onChange={(uploadMbps) =>
              patchHysteria({ uploadMbps: uploadMbps ?? 0 })
            }
            value={settings.hysteria.uploadMbps}
          />
          <NumberField
            field="hysteria.downloadMbps"
            id="rt-hysteria-down"
            label={t("settings.fields.hysteriaDownMbps")}
            onChange={(downloadMbps) =>
              patchHysteria({ downloadMbps: downloadMbps ?? 0 })
            }
            value={settings.hysteria.downloadMbps}
          />
          <NumberField
            field="hysteria.hopIntervalSeconds"
            id="rt-hysteria-hop-interval"
            label={t("settings.core.hysteriaHopInterval")}
            onChange={(hopIntervalSeconds) =>
              patchHysteria({ hopIntervalSeconds: hopIntervalSeconds ?? 5 })
            }
            value={settings.hysteria.hopIntervalSeconds}
          />
        </SettingsGroup>
      </Disclosure>
    </div>
  );
}
