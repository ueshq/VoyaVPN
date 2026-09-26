import { Server } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import type { ProfileKind } from "@voya/contracts";
import { CheckboxField, TextField } from "@voya/ui/components/form-fields";

import {
  Panel,
} from "./profile-form-fields";
import type {
  ProfileEditorForm,
  ProfileFieldErrors,
} from "./profile-editor-form";
import {
  passwordLabel,
  requiresUsername,
  usernameLabel,
} from "@voya/features/profiles/profile-form-utils";

type ProtocolPanelProps = {
  configType: ProfileKind;
  errors: ProfileFieldErrors;
  form: ProfileEditorForm;
  onFieldChange: (key: "password" | "username", value: string) => void;
  onOptionChange: <Key extends keyof ProfileEditorForm["protocolOptions"]>(
    key: Key,
    value: ProfileEditorForm["protocolOptions"][Key],
  ) => void;
};

export function ProtocolPanel({
  configType,
  errors,
  form,
  onFieldChange,
  onOptionChange,
}: ProtocolPanelProps) {
  const { t } = useI18n();
  const options = form.protocolOptions;

  return (
    <Panel icon={Server} title={t("panes.profiles.panels.protocol")}>
      <div className="grid gap-3 lg:grid-cols-3">
        {requiresUsername(configType) ? (
          <TextField
            error={errors.username}
            label={usernameLabel(configType, t)}
            onChange={(value) => onFieldChange("username", value)}
            value={form.username}
          />
        ) : null}
        <TextField
          error={errors.password}
          label={passwordLabel(configType, t)}
          onChange={(value) => onFieldChange("password", value)}
          value={form.password}
        />
        {configType === "vmess" ? (
          <TextField
            label={t("panes.profiles.fields.vmessSecurity")}
            onChange={(value) => onOptionChange("vmessCipher", value)}
            placeholder="auto"
            value={options.vmessCipher}
          />
        ) : null}
        {configType === "vless" ? (
          <>
            <TextField
              label={t("panes.profiles.fields.flow")}
              onChange={(value) => onOptionChange("flow", value)}
              placeholder="xtls-rprx-vision"
              value={options.flow}
            />
            <TextField
              label={t("panes.profiles.fields.encryption")}
              onChange={(value) => onOptionChange("vlessEncryption", value)}
              placeholder="none"
              value={options.vlessEncryption}
            />
          </>
        ) : null}
        {configType === "shadowsocks" ? (
          <>
            <TextField
              label={t("panes.profiles.fields.method")}
              onChange={(value) => onOptionChange("method", value)}
              placeholder="2022-blake3-aes-128-gcm"
              value={options.method}
            />
            <CheckboxField
              checked={options.udpOverTcp}
              label={t("panes.profiles.fields.udpOverTcp")}
              onChange={(checked) => onOptionChange("udpOverTcp", checked)}
            />
          </>
        ) : null}
        {configType === "hysteria2" ? (
          <>
            <TextField
              label={t("panes.profiles.fields.ports")}
              onChange={(value) => onOptionChange("portHops", value)}
              value={options.portHops}
            />
            <TextField
              label={t("panes.profiles.fields.salamanderPassword")}
              onChange={(value) => onOptionChange("obfuscationPassword", value)}
              value={options.obfuscationPassword}
            />
          </>
        ) : null}
        {configType === "tuic" ? (
          <TextField
            label={t("panes.profiles.fields.congestionControl")}
            onChange={(value) => onOptionChange("congestionControl", value)}
            placeholder="bbr"
            value={options.congestionControl}
          />
        ) : null}
        {configType === "wireGuard" ? (
          <>
            <TextField
              label={t("panes.profiles.fields.peerPublicKey")}
              onChange={(value) =>
                onOptionChange("wireGuardPeerPublicKey", value)
              }
              value={options.wireGuardPeerPublicKey}
            />
            <TextField
              label={t("panes.profiles.fields.presharedKey")}
              onChange={(value) =>
                onOptionChange("wireGuardPresharedKey", value)
              }
              value={options.wireGuardPresharedKey}
            />
            <TextField
              label={t("panes.profiles.fields.interfaceAddress")}
              onChange={(value) =>
                onOptionChange("wireGuardInterfaceAddress", value)
              }
              value={options.wireGuardInterfaceAddress}
            />
            <TextField
              label={t("panes.profiles.fields.allowedIps")}
              onChange={(value) => onOptionChange("wireGuardAllowedIps", value)}
              value={options.wireGuardAllowedIps}
            />
            <TextField
              label={t("panes.profiles.fields.reservedBytes")}
              onChange={(value) => onOptionChange("wireGuardReserved", value)}
              value={options.wireGuardReserved}
            />
            <TextField
              error={errors["protocolOptions.wireGuardMtu"]}
              inputMode="numeric"
              label={t("panes.profiles.fields.mtu")}
              onChange={(value) => onOptionChange("wireGuardMtu", value)}
              value={options.wireGuardMtu}
            />
          </>
        ) : null}
        {configType === "naive" ? (
          <>
            <CheckboxField
              checked={options.naiveQuic}
              label={t("panes.profiles.fields.quic")}
              onChange={(checked) => onOptionChange("naiveQuic", checked)}
            />
            <TextField
              label={t("panes.profiles.fields.congestionControl")}
              onChange={(value) => onOptionChange("congestionControl", value)}
              placeholder="bbr"
              value={options.congestionControl}
            />
            <TextField
              error={errors["protocolOptions.insecureConcurrency"]}
              inputMode="numeric"
              label={t("panes.profiles.fields.insecureConcurrency")}
              onChange={(value) =>
                onOptionChange("insecureConcurrency", value)
              }
              value={options.insecureConcurrency}
            />
            <CheckboxField
              checked={options.udpOverTcp}
              label={t("panes.profiles.fields.udpOverTcp")}
              onChange={(checked) => onOptionChange("udpOverTcp", checked)}
            />
          </>
        ) : null}
      </div>
    </Panel>
  );
}
