import { Server } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { CheckboxField } from "@voya/ui/components/form-fields";

import { DraftTextField, Panel, type ProfilePanelProps } from "./profile-form-fields";

/** The selected protocol's own fields; the other protocols' drafts stay hidden. */
export function ProtocolPanel(panel: ProfilePanelProps) {
  const { t } = useI18n();
  const { draft, onChange } = panel;
  const uuid = (
    <DraftTextField {...panel} label={t("panes.profiles.fields.uuid")} name="uuid" />
  );
  const username = (
    <DraftTextField {...panel} label={t("panes.profiles.fields.username")} name="username" />
  );
  const password = (
    <DraftTextField {...panel} label={t("panes.profiles.fields.password")} name="password" />
  );
  const congestionControl = (
    <DraftTextField
      {...panel}
      label={t("panes.profiles.fields.congestionControl")}
      name="congestionControl"
      placeholder="bbr"
    />
  );
  const udpOverTcp = (
    <CheckboxField
      checked={draft.udpOverTcp}
      label={t("panes.profiles.fields.udpOverTcp")}
      onChange={(checked) => onChange("udpOverTcp", checked)}
    />
  );

  function fields() {
    switch (draft.kind) {
      case "vmess":
        return (
          <>
            {uuid}
            <DraftTextField
              {...panel}
              label={t("panes.profiles.fields.vmessSecurity")}
              name="cipher"
              placeholder="auto"
            />
          </>
        );
      case "vless":
        return (
          <>
            {uuid}
            <DraftTextField
              {...panel}
              label={t("panes.profiles.fields.flow")}
              name="flow"
              placeholder="xtls-rprx-vision"
            />
            <DraftTextField
              {...panel}
              label={t("panes.profiles.fields.encryption")}
              name="encryption"
              placeholder="none"
            />
          </>
        );
      case "shadowsocks":
        return (
          <>
            {password}
            <DraftTextField
              {...panel}
              label={t("panes.profiles.fields.method")}
              name="method"
              placeholder="2022-blake3-aes-128-gcm"
            />
            {udpOverTcp}
          </>
        );
      case "trojan":
      case "anytls":
        return password;
      case "socks":
      case "http":
        return (
          <>
            {username}
            {password}
          </>
        );
      case "hysteria2":
        return (
          <>
            {password}
            <DraftTextField {...panel} label={t("panes.profiles.fields.ports")} name="portHops" />
            <DraftTextField
              {...panel}
              label={t("panes.profiles.fields.salamanderPassword")}
              name="obfuscationPassword"
            />
          </>
        );
      case "tuic":
        return (
          <>
            {uuid}
            {password}
            {congestionControl}
          </>
        );
      case "wireGuard":
        return (
          <>
            <DraftTextField {...panel} label={t("panes.profiles.fields.privateKey")} name="privateKey" />
            <DraftTextField {...panel} label={t("panes.profiles.fields.peerPublicKey")} name="peerPublicKey" />
            <DraftTextField {...panel} label={t("panes.profiles.fields.presharedKey")} name="presharedKey" />
            <DraftTextField
              {...panel}
              label={t("panes.profiles.fields.interfaceAddress")}
              name="interfaceAddress"
            />
            <DraftTextField {...panel} label={t("panes.profiles.fields.allowedIps")} name="allowedIps" />
            <DraftTextField {...panel} label={t("panes.profiles.fields.reservedBytes")} name="reserved" />
            <DraftTextField {...panel} inputMode="numeric" label={t("panes.profiles.fields.mtu")} name="mtu" />
          </>
        );
      case "naive":
        return (
          <>
            {username}
            {password}
            <CheckboxField
              checked={draft.quic}
              label={t("panes.profiles.fields.quic")}
              onChange={(checked) => onChange("quic", checked)}
            />
            {congestionControl}
            <DraftTextField
              {...panel}
              inputMode="numeric"
              label={t("panes.profiles.fields.insecureConcurrency")}
              name="insecureConcurrency"
            />
            {udpOverTcp}
          </>
        );
    }
  }

  return (
    <Panel icon={Server} title={t("panes.profiles.panels.protocol")}>
      <div className="grid gap-3 lg:grid-cols-3">{fields()}</div>
    </Panel>
  );
}
