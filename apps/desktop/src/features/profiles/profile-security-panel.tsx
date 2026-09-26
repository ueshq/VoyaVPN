import { ShieldCheck } from "lucide-react";

import { Disclosure } from "@voya/ui/components/disclosure";
import { SelectField, TextAreaField } from "@voya/ui/components/form-fields";
import { useI18n } from "@voya/i18n/use-i18n";

import { isTlsModeOption, TLS_MODE_OPTIONS } from "@voya/features/profiles/profile-constants";
import { DraftTextField, Panel, type ProfilePanelProps } from "./profile-form-fields";

export function SecurityPanel(panel: ProfilePanelProps) {
  const { t } = useI18n();
  const { draft, errors, onChange } = panel;
  const mode = draft.tlsMode;

  return (
    <Panel icon={ShieldCheck} title={t("panes.profiles.panels.security")}>
      <div className="grid gap-3 lg:grid-cols-4">
        <SelectField
          label={t("panes.profiles.fields.tlsMode")}
          onChange={(value) => {
            if (isTlsModeOption(value)) onChange("tlsMode", value);
          }}
          options={[{ label: t("common.none"), value: "none" }, ...TLS_MODE_OPTIONS]}
          value={mode}
        />
        {mode !== "none" ? (
          <>
            <DraftTextField {...panel} label={t("panes.profiles.fields.sni")} name="serverName" />
            <DraftTextField {...panel} label={t("panes.profiles.fields.alpn")} name="alpn" />
            {mode === "reality" ? (
              <>
                <DraftTextField
                  {...panel}
                  label={t("panes.profiles.fields.realityPublicKey")}
                  name="realityPublicKey"
                />
                <DraftTextField {...panel} label={t("panes.profiles.fields.shortId")} name="realityShortId" />
              </>
            ) : null}
            <Disclosure title={t("common.advanced")} className="lg:col-span-4">
              <DraftTextField {...panel} label={t("panes.profiles.fields.echConfigList")} name="echConfig" />
              <TextAreaField
                error={errors.certificatePem}
                inputClassName="font-mono text-xs"
                label={t("panes.profiles.fields.pinnedCert")}
                onChange={(value) => onChange("certificatePem", value)}
                value={draft.certificatePem}
              />
            </Disclosure>
          </>
        ) : null}
      </div>
    </Panel>
  );
}
