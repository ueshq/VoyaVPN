import type { UseFormGetValues, UseFormSetValue } from "react-hook-form";

import { useI18n } from "@voya/i18n/use-i18n";
import { GroupBuilder } from "@/features/groups/group-builder";

import { CONFIG_TYPES, type ProfileProtocol } from "./profile-constants";
import {
  CheckboxField,
  Panel,
  TextField,
  type ProfileFormControl,
  type Register,
} from "./profile-form-fields";
import { passwordLabel, requiresUsername, optionalNumber, usernameLabel } from "./profile-form-utils";
import type { ProfileFormValues } from "./profile-form-schema";

type ProtocolPanelProps = {
  configType: ProfileProtocol;
  control: ProfileFormControl;
  getValues: UseFormGetValues<ProfileFormValues>;
  passwordError?: string;
  register: Register;
  setValue: UseFormSetValue<ProfileFormValues>;
  usernameError?: string;
};

export function ProtocolPanel({
  configType,
  control,
  getValues,
  passwordError,
  register,
  setValue,
  usernameError,
}: ProtocolPanelProps) {
  const { t } = useI18n();

  if (configType === CONFIG_TYPES.PolicyGroup || configType === CONFIG_TYPES.ProxyChain) {
    return (
      <Panel
        title={
          configType === CONFIG_TYPES.PolicyGroup
            ? t("panes.profiles.panels.policyGroup")
            : t("panes.profiles.panels.proxyChain")
        }
      >
        <GroupBuilder
          configType={configType}
          control={control}
          getValues={getValues}
          register={register}
          setValue={setValue}
        />
      </Panel>
    );
  }

  if (configType === CONFIG_TYPES.Custom) {
    // `address` is the config source for custom profiles and is already edited
    // in the Profile panel; registering a second input for it would leave two
    // fields fighting over one react-hook-form ref.
    return (
      <Panel title={t("panes.profiles.panels.custom")}>
        <div className="grid gap-3 lg:grid-cols-2">
          <TextField label={t("panes.profiles.fields.filter")} {...register("protocolOptions.filter")} />
        </div>
      </Panel>
    );
  }

  return (
    <Panel title={t("panes.profiles.panels.protocol")}>
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
