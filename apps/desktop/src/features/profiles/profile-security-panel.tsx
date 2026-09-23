import { ShieldCheck } from "lucide-react";

import { Disclosure } from "@voya/ui/components/disclosure";
import { SelectField, TextAreaField, TextField } from "@voya/ui/components/form-fields";
import { useI18n } from "@voya/i18n/use-i18n";

import { SECURITY_OPTIONS } from "@voya/features/profiles/profile-constants";
import {
  Panel,
} from "./profile-form-fields";
import type {
  ProfileEditorForm,
  ProfileFieldErrors,
} from "./profile-editor-form";

type SecurityPanelProps = {
  errors: ProfileFieldErrors;
  form: ProfileEditorForm;
  onFieldChange: <
    Key extends
      | "streamSecurity"
      | "sni"
      | "alpn"
      | "publicKey"
      | "shortId"
      | "cert"
      | "echConfigList",
  >(
    key: Key,
    value: string,
  ) => void;
  security: string;
};

export function SecurityPanel({
  errors,
  form,
  onFieldChange,
  security,
}: SecurityPanelProps) {
  const { t } = useI18n();
  const reality = security === "reality";

  return (
    <Panel icon={ShieldCheck} title={t("panes.profiles.panels.security")}>
      <div className="grid gap-3 lg:grid-cols-4">
        <SelectField
          label={t("panes.profiles.fields.tlsMode")}
          onChange={(value) => onFieldChange("streamSecurity", value)}
          options={SECURITY_OPTIONS.map((option) => ({
            ...option,
            label: option.value ? option.label : t("common.none"),
          }))}
          value={form.streamSecurity}
        />
        {security ? (
          <>
            <TextField
              label={t("panes.profiles.fields.sni")}
              onChange={(value) => onFieldChange("sni", value)}
              value={form.sni}
            />
            <TextField
              label={t("panes.profiles.fields.alpn")}
              onChange={(value) => onFieldChange("alpn", value)}
              value={form.alpn}
            />
            {reality ? (
              <>
                <TextField
                  label={t("panes.profiles.fields.realityPublicKey")}
                  onChange={(value) => onFieldChange("publicKey", value)}
                  value={form.publicKey}
                />
                <TextField
                  label={t("panes.profiles.fields.shortId")}
                  onChange={(value) => onFieldChange("shortId", value)}
                  value={form.shortId}
                />
              </>
            ) : null}
            <Disclosure title={t("common.advanced")} className="lg:col-span-4">
              <TextField
                label={t("panes.profiles.fields.echConfigList")}
                onChange={(value) => onFieldChange("echConfigList", value)}
                value={form.echConfigList}
              />
              <TextAreaField
                error={errors.cert}
                inputClassName="font-mono text-xs"
                label={t("panes.profiles.fields.pinnedCert")}
                onChange={(value) => onFieldChange("cert", value)}
                value={form.cert}
              />
            </Disclosure>
          </>
        ) : null}
      </div>
    </Panel>
  );
}
