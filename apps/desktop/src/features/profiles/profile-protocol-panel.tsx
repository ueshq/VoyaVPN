
import { Server } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";

import { CONFIG_TYPES, type ProfileProtocol } from "./profile-constants";
import {
  CheckboxField,
  Panel,
  TextField,
  type ProfileFormControl,
  type Register,
} from "./profile-form-fields";
import { passwordLabel, requiresUsername, optionalNumber, usernameLabel } from "./profile-form-utils";

type ProtocolPanelProps = {
  configType: ProfileProtocol;
  control: ProfileFormControl;
  passwordError?: string;
  register: Register;
  usernameError?: string;
};

export function ProtocolPanel({
  configType,
  control,
  passwordError,
  register,
  usernameError,
}: ProtocolPanelProps) {
  const { t } = useI18n();

  return (
    <Panel icon={Server} title={t("panes.profiles.panels.protocol")}>
      <div className="grid gap-3 lg:grid-cols-3">
        {requiresUsername(configType) ? (
          <TextField
            error={usernameError}
            label={usernameLabel(configType, t)}
            {...register("username")}
          />
        ) : null}
        <TextField error={passwordError} label={passwordLabel(configType, t)} {...register("password")} />
        {configType === CONFIG_TYPES.VMess ? (
          <>
            <TextField label={t("panes.profiles.fields.vmessSecurity")} placeholder="auto" {...register("protocolOptions.vmessCipher")} />
          </>
        ) : null}
        {configType === CONFIG_TYPES.VLESS ? (
          <>
            <TextField label={t("panes.profiles.fields.flow")} placeholder="xtls-rprx-vision" {...register("protocolOptions.flow")} />
            <TextField label={t("panes.profiles.fields.encryption")} placeholder="none" {...register("protocolOptions.vlessEncryption")} />
          </>
        ) : null}
        {configType === CONFIG_TYPES.Shadowsocks ? (
          <>
            <TextField label={t("panes.profiles.fields.method")} placeholder="2022-blake3-aes-128-gcm" {...register("protocolOptions.method")} />
            <CheckboxField control={control} label={t("panes.profiles.fields.udpOverTcp")} name="protocolOptions.udpOverTcp" />
          </>
        ) : null}
        {configType === CONFIG_TYPES.Hysteria2 ? (
          <>
            <TextField label={t("panes.profiles.fields.ports")} {...register("protocolOptions.portHops")} />
            <TextField label={t("panes.profiles.fields.salamanderPassword")} {...register("protocolOptions.obfuscationPassword")} />
          </>
        ) : null}
        {configType === CONFIG_TYPES.TUIC ? (
          <TextField label={t("panes.profiles.fields.congestionControl")} placeholder="bbr" {...register("protocolOptions.congestionControl")} />
        ) : null}
        {configType === CONFIG_TYPES.WireGuard ? (
          <>
            <TextField label={t("panes.profiles.fields.peerPublicKey")} {...register("protocolOptions.wireGuardPeerPublicKey")} />
            <TextField label={t("panes.profiles.fields.presharedKey")} {...register("protocolOptions.wireGuardPresharedKey")} />
            <TextField label={t("panes.profiles.fields.interfaceAddress")} {...register("protocolOptions.wireGuardInterfaceAddress")} />
            <TextField label={t("panes.profiles.fields.allowedIps")} {...register("protocolOptions.wireGuardAllowedIps")} />
            <TextField label={t("panes.profiles.fields.reservedBytes")} {...register("protocolOptions.wireGuardReserved")} />
            <TextField
              inputMode="numeric"
              label={t("panes.profiles.fields.mtu")}
              type="number"
              {...register("protocolOptions.wireGuardMtu", { setValueAs: optionalNumber })}
            />
          </>
        ) : null}
        {configType === CONFIG_TYPES.Naive ? (
          <>
            <CheckboxField control={control} label={t("panes.profiles.fields.quic")} name="protocolOptions.naiveQuic" />
            <TextField label={t("panes.profiles.fields.congestionControl")} placeholder="bbr" {...register("protocolOptions.congestionControl")} />
            <TextField
              inputMode="numeric"
              label={t("panes.profiles.fields.insecureConcurrency")}
              type="number"
              {...register("protocolOptions.insecureConcurrency", { setValueAs: optionalNumber })}
            />
            <CheckboxField control={control} label={t("panes.profiles.fields.udpOverTcp")} name="protocolOptions.udpOverTcp" />
          </>
        ) : null}
      </div>
    </Panel>
  );
}
