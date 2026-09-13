import { useI18n } from "@voya/i18n/use-i18n";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";

import {
  NumberField,
  SettingsCheckbox,
  SettingsGroup,
  TextField,
} from "./settings-form";
import { SETTING_DEFAULTS } from "./settings-values";
import type { AppSettingsFormController } from "./use-app-settings";

/**
 * Connection choices an ordinary user makes: the kill switch and the local
 * proxy port. DNS follows on the same tab.
 */
export function ConnectionTab({
  controller,
}: {
  controller: AppSettingsFormController;
}) {
  const { t } = useI18n();
  const { settings, update } = controller;
  // Where the system proxy can be chosen instead, the kill switch only covers
  // VPN mode.
  const systemProxyMode = useRuntimeEventStore(
    (state) =>
      state.sysProxy?.management === "automatic" && state.tun?.enabled === false,
  );

  const inbound = settings.network.inbounds[0];
  const patchInbound = (
    patch: Partial<(typeof settings.network.inbounds)[number]>,
  ) =>
    update((current) => ({
      ...current,
      network: {
        ...current.network,
        inbounds: current.network.inbounds.map((item, index) =>
          index === 0 ? { ...item, ...patch } : item,
        ),
      },
    }));

  return (
    <div className="grid gap-4">
      <SettingsGroup title={t("settings.sections.killSwitch")}>
        {/* The tunnel's strict route is what keeps traffic from leaving
            around it, on every platform. Outside VPN mode there is nothing
            to protect, so it cannot be changed there. */}
        <SettingsCheckbox
          checked={settings.network.tun.strictRoute}
          disabled={systemProxyMode}
          description={t(
            systemProxyMode
              ? "settings.killSwitch.vpnOnly"
              : "settings.killSwitch.hint",
          )}
          field="network.tun.strictRoute"
          label={t("settings.killSwitch.label")}
          onCheckedChange={(strictRoute) =>
            update((current) => ({
              ...current,
              network: {
                ...current.network,
                tun: {
                  ...current.network.tun,
                  strictRoute: strictRoute === true,
                },
              },
            }))
          }
        />
      </SettingsGroup>

      {inbound ? (
        <SettingsGroup title={t("settings.sections.localProxy")}>
          <NumberField
            defaultValue={SETTING_DEFAULTS.localPort}
            description={t("settings.network.localPortHint", {
              lan: inbound.localPort + 2,
              second: inbound.localPort + 1,
            })}
            field="network.inbounds.0.localPort"
            id="rt-inbound-port"
            label={t("settings.network.localPort")}
            onChange={(localPort) =>
              patchInbound({
                localPort: localPort ?? SETTING_DEFAULTS.localPort,
              })
            }
            value={inbound.localPort}
          />
          <div className="grid gap-3 @min-[42rem]:grid-cols-2">
            <SettingsCheckbox
              checked={inbound.sniffingEnabled}
              description={t("settings.network.sniffingHint")}
              field="network.inbounds.0.sniffingEnabled"
              label={t("settings.network.sniffing")}
              onCheckedChange={(sniffingEnabled) =>
                patchInbound({ sniffingEnabled: sniffingEnabled === true })
              }
            />
            <SettingsCheckbox
              checked={inbound.secondaryPortEnabled}
              description={t("settings.network.secondPortHint", {
                port: inbound.localPort + 1,
              })}
              field="network.inbounds.0.secondaryPortEnabled"
              label={t("settings.network.secondPort")}
              onCheckedChange={(secondaryPortEnabled) =>
                patchInbound({
                  secondaryPortEnabled: secondaryPortEnabled === true,
                })
              }
            />
            <SettingsCheckbox
              checked={inbound.lanConnectionsAllowed}
              description={t("settings.network.allowLanHint")}
              field="network.inbounds.0.lanConnectionsAllowed"
              label={t("settings.network.allowLan")}
              onCheckedChange={(lanConnectionsAllowed) =>
                patchInbound({
                  lanConnectionsAllowed: lanConnectionsAllowed === true,
                })
              }
            />
            {inbound.lanConnectionsAllowed ? (
              <SettingsCheckbox
                checked={inbound.separateLanPort}
                description={t("settings.network.separateLanPortHint", {
                  port: inbound.localPort + 2,
                })}
                field="network.inbounds.0.separateLanPort"
                label={t("settings.network.separateLanPort")}
                onCheckedChange={(separateLanPort) =>
                  patchInbound({ separateLanPort: separateLanPort === true })
                }
              />
            ) : null}
          </div>
          {inbound.lanConnectionsAllowed && inbound.separateLanPort ? (
            <>
              <TextField
                field="network.inbounds.0.username"
                id="rt-inbound-username"
                label={t("settings.network.lanUsername")}
                onChange={(username) => patchInbound({ username })}
                value={inbound.username}
              />
              <TextField
                field="network.inbounds.0.password"
                id="rt-inbound-password"
                label={t("settings.network.lanPassword")}
                onChange={(password) => patchInbound({ password })}
                type="password"
                value={inbound.password}
              />
            </>
          ) : null}
        </SettingsGroup>
      ) : null}
    </div>
  );
}
