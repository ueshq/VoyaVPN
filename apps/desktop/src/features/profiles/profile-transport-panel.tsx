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

type TransportPanelProps = {
  control: ProfileFormControl;
  register: Register;
};

export function TransportPanel({ control, register }: TransportPanelProps) {
  const { t } = useI18n();
  // Only the selected network's options are rendered, so no two inputs ever
  // fight over one react-hook-form ref.
  const network = useWatch({ control, name: "network" }) || "tcp";
  const options = useWatch({ control, name: "transportOptions" });
  // Plain TCP with nothing filled in has nothing to show, so it starts closed.
  const hasSettings =
    network !== "tcp" ||
    Object.values(options ?? {}).some((value) =>
      typeof value === "string" ? value.trim() !== "" && value !== "none" : Boolean(value),
    );

  return (
    <Panel collapsible defaultOpen={hasSettings} title={t("panes.profiles.panels.transport")}>
      <div className="grid gap-3 lg:grid-cols-4">
        <SelectField
          control={control}
          label={t("panes.profiles.fields.network")}
          name="network"
          options={NETWORK_OPTIONS}
        />
        {["tcp", "ws", "httpupgrade", "h2", "quic"].includes(network) ? (
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
      </div>
    </Panel>
  );
}
