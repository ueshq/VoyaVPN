import { Waypoints } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { SelectField } from "@voya/ui/components/form-fields";

import { isTransportKind, TRANSPORT_OPTIONS } from "@voya/features/profiles/profile-constants";
import { DraftTextField, Panel, type ProfilePanelProps } from "./profile-form-fields";

export function TransportPanel(panel: ProfilePanelProps) {
  const { t } = useI18n();
  const { draft, onChange } = panel;
  // Only the selected transport's settings are rendered, so no two inputs ever
  // share one name.
  const transport = draft.transport;
  // Plain TCP with nothing filled in has nothing to show, so it starts closed.
  const hasSettings =
    transport !== "tcp" ||
    [draft.header, draft.host, draft.path, draft.authority, draft.serviceName, draft.mode].some(
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
          onChange={(value) => {
            if (isTransportKind(value)) onChange("transport", value);
          }}
          options={TRANSPORT_OPTIONS}
          value={transport}
        />
        {transport === "grpc" ? (
          <>
            <DraftTextField {...panel} label={t("panes.profiles.fields.grpcAuthority")} name="authority" />
            <DraftTextField {...panel} label={t("panes.profiles.fields.grpcService")} name="serviceName" />
            <DraftTextField {...panel} label={t("panes.profiles.fields.grpcMode")} name="mode" />
          </>
        ) : (
          <>
            <DraftTextField {...panel} label={t("panes.profiles.fields.host")} name="host" />
            <DraftTextField {...panel} label={t("panes.profiles.fields.path")} name="path" />
          </>
        )}
        {transport === "tcp" ? (
          <DraftTextField
            {...panel}
            label={t("panes.profiles.fields.rawHeader")}
            name="header"
            placeholder="none"
          />
        ) : null}
      </div>
    </Panel>
  );
}
