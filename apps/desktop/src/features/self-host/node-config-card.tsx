import { useI18n } from "@voya/i18n/use-i18n";

import {
  NumberField,
  SettingsFields,
  SettingsGroup,
  SettingsSwitch,
  TextField,
} from "@/features/settings/settings-form";
import type { SelfHostState } from "@/ipc/bindings";

import type { SelfHostController } from "./use-self-host";

/**
 * The node's settings. Every change saves at once; a change that alters the
 * server (a port, a protocol, the disguise site) restarts it, which drops the
 * connections of devices using it for a moment.
 */
export function NodeConfigCard({
  controller,
  state,
}: {
  controller: SelfHostController;
  state: SelfHostState;
}) {
  const { t } = useI18n();
  const { config } = state;
  const save = controller.saveConfig;
  const portHint = (port: number) =>
    port === 0 ? t("panes.selfHost.config.portAuto") : t("panes.selfHost.config.portHint");

  return (
    <SettingsFields errors={controller.fieldErrors}>
      <SettingsGroup title={t("panes.selfHost.config.title")}>
        <TextField
          description={t("panes.selfHost.config.deviceLabelHint")}
          field="deviceLabel"
          id="self-host-device-label"
          label={t("panes.selfHost.config.deviceLabel")}
          onChange={(deviceLabel) => void save({ deviceLabel })}
          value={config.deviceLabel}
        />
        <SettingsSwitch
          checked={config.vlessEnabled}
          description={t("panes.selfHost.config.vlessHint")}
          field="protocols"
          label={t("panes.selfHost.config.vless")}
          onCheckedChange={(vlessEnabled) => void save({ vlessEnabled })}
        />
        <NumberField
          description={portHint(config.vlessPort)}
          disabled={!config.vlessEnabled}
          field="vlessPort"
          id="self-host-vless-port"
          label={t("panes.selfHost.config.vlessPort")}
          nullable
          onChange={(port) => void save({ vlessPort: port ?? 0 })}
          value={config.vlessPort === 0 ? null : config.vlessPort}
        />
        <TextField
          description={t("panes.selfHost.config.realityServerHint")}
          disabled={!config.vlessEnabled}
          field="realityServerName"
          id="self-host-reality-server"
          label={t("panes.selfHost.config.realityServer")}
          onChange={(realityServerName) => void save({ realityServerName })}
          value={config.realityServerName}
        />
        <NumberField
          disabled={!config.vlessEnabled}
          field="realityServerPort"
          id="self-host-reality-port"
          label={t("panes.selfHost.config.realityServerPort")}
          onChange={(port) => void save({ realityServerPort: port ?? 443 })}
          value={config.realityServerPort}
        />
        <SettingsSwitch
          checked={config.shadowsocksEnabled}
          description={t("panes.selfHost.config.shadowsocksHint")}
          label={t("panes.selfHost.config.shadowsocks")}
          onCheckedChange={(shadowsocksEnabled) => void save({ shadowsocksEnabled })}
        />
        <NumberField
          description={portHint(config.shadowsocksPort)}
          disabled={!config.shadowsocksEnabled}
          field="shadowsocksPort"
          id="self-host-shadowsocks-port"
          label={t("panes.selfHost.config.shadowsocksPort")}
          nullable
          onChange={(port) => void save({ shadowsocksPort: port ?? 0 })}
          value={config.shadowsocksPort === 0 ? null : config.shadowsocksPort}
        />
        <TextField
          description={t("panes.selfHost.config.customAddressHint")}
          field="customAddress"
          id="self-host-custom-address"
          label={t("panes.selfHost.config.customAddress")}
          onChange={(address) => void save({ customAddress: address.trim() || null })}
          value={config.customAddress ?? ""}
        />
        <SettingsSwitch
          checked={config.upnpEnabled}
          description={t("panes.selfHost.config.upnpHint")}
          label={t("panes.selfHost.config.upnp")}
          onCheckedChange={(upnpEnabled) => void save({ upnpEnabled })}
        />
        <SettingsSwitch
          checked={config.allowLanAccess}
          description={t("panes.selfHost.config.allowLanHint")}
          label={t("panes.selfHost.config.allowLan")}
          onCheckedChange={(allowLanAccess) => void save({ allowLanAccess })}
        />
        <SettingsSwitch
          checked={config.blockBittorrent}
          description={t("panes.selfHost.config.blockBittorrentHint")}
          label={t("panes.selfHost.config.blockBittorrent")}
          onCheckedChange={(blockBittorrent) => void save({ blockBittorrent })}
        />
      </SettingsGroup>
    </SettingsFields>
  );
}
