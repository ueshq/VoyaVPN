import { Waypoints } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { SelectField, TextField } from "@voya/ui/components/form-fields";

import { NETWORK_OPTIONS } from "@voya/features/profiles/profile-constants";
import {
  Panel,
} from "./profile-form-fields";
import type {
  ProfileEditorForm,
  ProfileFieldErrors,
} from "./profile-editor-form";

type TransportPanelProps = {
  errors: ProfileFieldErrors;
  form: ProfileEditorForm;
  onFieldChange: (key: "network", value: string) => void;
  onTransportChange: <Key extends keyof ProfileEditorForm["transportOptions"]>(
    key: Key,
    value: ProfileEditorForm["transportOptions"][Key],
  ) => void;
};

export function TransportPanel({
  errors,
  form,
  onFieldChange,
  onTransportChange,
}: TransportPanelProps) {
  const { t } = useI18n();
  // Only the selected network's options are rendered, so no two inputs ever
  // share one name.
  const network = form.network || "tcp";
  const options = form.transportOptions;
  // Plain TCP with nothing filled in has nothing to show, so it starts closed.
  const hasSettings =
    network !== "tcp" ||
    Object.values(options).some(
      (value) => value.trim() !== "" && value !== "none",
    );

  return (
    <Panel
      collapsible
      defaultOpen={hasSettings}
      icon={Waypoints}
      title={t("panes.profiles.panels.transport")}
    >
      <div className="grid gap-3 lg:grid-cols-4">
        <SelectField
          label={t("panes.profiles.fields.network")}
          onChange={(value) => onFieldChange("network", value)}
          options={NETWORK_OPTIONS}
          value={network}
        />
        {["tcp", "ws", "httpupgrade", "h2", "quic"].includes(network) ? (
          <>
            <TextField
              label={t("panes.profiles.fields.host")}
              onChange={(value) => onTransportChange("host", value)}
              value={options.host}
            />
            <TextField
              label={t("panes.profiles.fields.path")}
              onChange={(value) => onTransportChange("path", value)}
              value={options.path}
            />
          </>
        ) : null}
        {network === "tcp" ? (
          <TextField
            label={t("panes.profiles.fields.rawHeader")}
            onChange={(value) => onTransportChange("header", value)}
            placeholder="none"
            value={options.header}
          />
        ) : null}
        {network === "grpc" ? (
          <>
            <TextField
              label={t("panes.profiles.fields.grpcAuthority")}
              onChange={(value) => onTransportChange("grpcAuthority", value)}
              value={options.grpcAuthority}
            />
            <TextField
              label={t("panes.profiles.fields.grpcService")}
              onChange={(value) => onTransportChange("grpcServiceName", value)}
              value={options.grpcServiceName}
            />
            <TextField
              label={t("panes.profiles.fields.grpcMode")}
              onChange={(value) => onTransportChange("grpcMode", value)}
              value={options.grpcMode}
            />
          </>
        ) : null}
        {errors.network ? (
          <span className="text-xs text-danger">{errors.network}</span>
        ) : null}
      </div>
    </Panel>
  );
}
