import { useWatch } from "react-hook-form";

import { useI18n } from "@voya/i18n/use-i18n";

import { NETWORK_OPTIONS } from "./profile-constants";
import {
  Panel,
  SelectField,
  TextField,
  type ProfileFormControl,
  type Register,
} from "./profile-form-fields";
import { optionalNumber } from "./profile-form-utils";

type TransportPanelProps = {
  control: ProfileFormControl;
  register: Register;
};

export function TransportPanel({ control, register }: TransportPanelProps) {
  const { t } = useI18n();
  // The raw (TCP) and KCP transports share `transportOptions.header`. Mounting
  // both inputs at once left two DOM nodes fighting over a single
  // react-hook-form ref, so only the selected network's header is rendered.
  const network = useWatch({ control, name: "network" }) || "tcp";

  return (
    <Panel title={t("panes.profiles.panels.transport")}>
      <div className="grid gap-3 lg:grid-cols-4">
        <SelectField
          control={control}
          label={t("panes.profiles.fields.network")}
          name="network"
          options={NETWORK_OPTIONS}
        />
        {["tcp", "ws", "httpupgrade", "h2", "quic", "xhttp"].includes(
          network,
        ) ? (
          <>
            <TextField
              label={t("panes.profiles.fields.host")}
              {...register("transportOptions.host")}
            />
            <TextField
              label={t("panes.profiles.fields.path")}
              {...register("transportOptions.path")}
            />
          </>
        ) : null}
        {network === "tcp" ? (
          <TextField
            label={t("panes.profiles.fields.rawHeader")}
            placeholder="none"
            {...register("transportOptions.header")}
          />
        ) : null}
        {network === "xhttp" ? (
          <>
            <TextField
              label={t("panes.profiles.fields.xhttpMode")}
              {...register("transportOptions.xhttpMode")}
            />
            <TextField
              label={t("panes.profiles.fields.xhttpExtra")}
              {...register("transportOptions.xhttpExtra")}
            />
          </>
        ) : null}
        {network === "grpc" ? (
          <>
            <TextField
              label={t("panes.profiles.fields.grpcAuthority")}
              {...register("transportOptions.grpcAuthority")}
            />
            <TextField
              label={t("panes.profiles.fields.grpcService")}
              {...register("transportOptions.grpcServiceName")}
            />
            <TextField
              label={t("panes.profiles.fields.grpcMode")}
              {...register("transportOptions.grpcMode")}
            />
          </>
        ) : null}
        {network === "kcp" ? (
          <>
            <TextField
              label={t("panes.profiles.fields.kcpHeader")}
              {...register("transportOptions.header")}
            />
            <TextField
              label={t("panes.profiles.fields.kcpSeed")}
              {...register("transportOptions.kcpSeed")}
            />
            <TextField
              inputMode="numeric"
              label={t("panes.profiles.fields.kcpMtu")}
              type="number"
              {...register("transportOptions.kcpMtu", {
                setValueAs: optionalNumber,
              })}
            />
          </>
        ) : null}
      </div>
    </Panel>
  );
}
