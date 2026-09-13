import { Disclosure } from "@voya/ui/components/disclosure";
import { nullableText, SETTING_DEFAULTS } from "./settings-values";
import {
  SettingsCheckbox,
  NumberField,
  SelectField,
  TextField,
  SettingsGroup,
  SettingsRow,
} from "./settings-form";
import type { TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";

import type { TlsFragmentMode } from "@/ipc/bindings";

import type { AppSettingsFormController } from "./use-app-settings";

const TLS_FRAGMENT_MODES = ["off", "tlsHello", "record"] as const satisfies readonly TlsFragmentMode[];
const TLS_FRAGMENT_LABELS: Record<TlsFragmentMode, TranslationKey> = {
  off: "settings.core.tlsFragmentOff",
  record: "settings.core.tlsFragmentRecord",
  tlsHello: "settings.core.tlsFragmentHello",
};
const LOG_LEVELS = ["none", "trace", "debug", "info", "warn", "error"] as const;
const LOG_LEVEL_LABELS: Record<(typeof LOG_LEVELS)[number], TranslationKey> = {
  none: "common.none",
  trace: "panes.logs.levels.trace",
  debug: "panes.logs.levels.debug",
  info: "panes.logs.levels.info",
  warn: "panes.logs.levels.warn",
  error: "panes.logs.levels.error",
};
// sing-box accepts exactly these, and the node's server must use the same one.
const MUX_PROTOCOLS = ["h2mux", "smux", "yamux"] as const;
// The contract's default, shown for a stored empty value.
const DEFAULT_MUX_PROTOCOL = "h2mux";

/** A protocol saved before this became a list stays selectable. */
function muxProtocolOptions(current: string): readonly string[] {
  return current && !(MUX_PROTOCOLS as readonly string[]).includes(current)
    ? [...MUX_PROTOCOLS, current]
    : MUX_PROTOCOLS;
}
// Only these fields live in the collapsed section, so only their errors open it.
const COLLAPSED_FIELDS = [
  "core.defaultFingerprint",
  "core.defaultUserAgent",
  "core.sendThrough",
  "core.bindInterface",
  "multiplexing.",
  "hysteria.",
];

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
            description={t("settings.core.allowInsecureHint")}
            label={t("settings.core.allowInsecure")}
            onCheckedChange={(defaultAllowInsecure) =>
              patchCore({ defaultAllowInsecure: defaultAllowInsecure === true })
            }
          />
          <SettingsCheckbox
            field="core.muxEnabled"
            checked={settings.core.muxEnabled}
            description={t("settings.core.muxEnabledHint")}
            label={t("settings.core.muxEnabled")}
            onCheckedChange={(muxEnabled) =>
              patchCore({ muxEnabled: muxEnabled === true })
            }
          />
          <SettingsCheckbox
            field="core.cacheFileEnabled"
            checked={settings.core.cacheFileEnabled}
            description={t("settings.core.cacheFileHint")}
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
          optionLabel={(level) =>
            t(LOG_LEVEL_LABELS[level as (typeof LOG_LEVELS)[number]])
          }
          options={LOG_LEVELS}
          value={settings.core.logLevel}
        />
        <SelectField
          field="core.tlsFragment"
          id="rt-tls-fragment"
          label={t("settings.core.tlsFragment")}
          onChange={(tlsFragment) =>
            patchCore({ tlsFragment: tlsFragment as TlsFragmentMode })
          }
          optionLabel={(mode) => t(TLS_FRAGMENT_LABELS[mode as TlsFragmentMode])}
          options={TLS_FRAGMENT_MODES}
          value={settings.core.tlsFragment}
        />
        {settings.core.tlsFragment === "tlsHello" ? (
          <NumberField
            defaultValue={SETTING_DEFAULTS.fragmentFallbackDelayMs}
            field="core.fragmentFallbackDelayMs"
            id="rt-fragment-fallback-delay"
            label={t("settings.core.fragmentFallbackDelay")}
            onChange={(delay) =>
              patchCore({
                fragmentFallbackDelayMs:
                  delay ?? SETTING_DEFAULTS.fragmentFallbackDelayMs,
              })
            }
            value={settings.core.fragmentFallbackDelayMs}
          />
        ) : null}
      </SettingsGroup>

      <Disclosure
        className="border-0 [&>div]:border-0"
        title={t("common.advanced")}
        invalid={Object.keys(controller.fieldErrors).some((field) =>
          COLLAPSED_FIELDS.some((prefix) => field.startsWith(prefix)),
        )}
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
          <SelectField
            description={t("settings.core.muxProtocolHint")}
            field="multiplexing.protocol"
            id="rt-mux-sbox-protocol"
            label={t("settings.core.muxProtocol")}
            onChange={(protocol) => patchMux({ protocol })}
            options={muxProtocolOptions(settings.multiplexing.protocol)}
            value={settings.multiplexing.protocol || DEFAULT_MUX_PROTOCOL}
          />
          <NumberField
            defaultValue={SETTING_DEFAULTS.muxMaxConnections}
            field="multiplexing.maxConnections"
            id="rt-mux-sbox-max-connections"
            label={t("settings.fields.muxMaxConnections")}
            onChange={(maxConnections) =>
              patchMux({
                maxConnections:
                  maxConnections ?? SETTING_DEFAULTS.muxMaxConnections,
              })
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
            defaultValue={SETTING_DEFAULTS.hysteriaMbps}
            field="hysteria.uploadMbps"
            id="rt-hysteria-up"
            label={t("settings.fields.hysteriaUpMbps")}
            onChange={(uploadMbps) =>
              patchHysteria({
                uploadMbps: uploadMbps ?? SETTING_DEFAULTS.hysteriaMbps,
              })
            }
            value={settings.hysteria.uploadMbps}
          />
          <NumberField
            defaultValue={SETTING_DEFAULTS.hysteriaMbps}
            field="hysteria.downloadMbps"
            id="rt-hysteria-down"
            label={t("settings.fields.hysteriaDownMbps")}
            onChange={(downloadMbps) =>
              patchHysteria({
                downloadMbps: downloadMbps ?? SETTING_DEFAULTS.hysteriaMbps,
              })
            }
            value={settings.hysteria.downloadMbps}
          />
          <NumberField
            defaultValue={SETTING_DEFAULTS.hysteriaHopIntervalSeconds}
            field="hysteria.hopIntervalSeconds"
            id="rt-hysteria-hop-interval"
            label={t("settings.core.hysteriaHopInterval")}
            onChange={(hopIntervalSeconds) =>
              patchHysteria({
                hopIntervalSeconds:
                  hopIntervalSeconds ??
                  SETTING_DEFAULTS.hysteriaHopIntervalSeconds,
              })
            }
            value={settings.hysteria.hopIntervalSeconds}
          />
        </SettingsGroup>
      </Disclosure>
    </div>
  );
}
