import { Disclosure } from "@voya/ui/components/disclosure";
import { Label } from "@voya/ui/components/label";
import { Textarea } from "@voya/ui/components/textarea";
import { useI18n } from "@voya/i18n/use-i18n";

import { SECURITY_OPTIONS } from "./profile-constants";
import {
  Panel,
  SelectField,
  TextField,
  type ProfileFormControl,
  type Register,
} from "./profile-form-fields";

type SecurityPanelProps = {
  control: ProfileFormControl;
  register: Register;
  security: string;
};

export function SecurityPanel({
  control,
  register,
  security,
}: SecurityPanelProps) {
  const { t } = useI18n();
  const reality = security === "reality";

  return (
    <Panel title={t("panes.profiles.panels.security")}>
      <div className="grid gap-3 lg:grid-cols-4">
        <SelectField
          control={control}
          label={t("panes.profiles.fields.tlsMode")}
          name="streamSecurity"
          options={SECURITY_OPTIONS.map((option) => ({ ...option, label: option.value ? option.label : t("common.none") }))}
        />
        {security ? (
          <>
            <TextField
              label={t("panes.profiles.fields.sni")}
              {...register("sni")}
            />
            <TextField
              label={t("panes.profiles.fields.alpn")}
              {...register("alpn")}
            />
            {reality ? (
              <>
                <TextField
                  label={t("panes.profiles.fields.realityPublicKey")}
                  {...register("publicKey")}
                />
                <TextField
                  label={t("panes.profiles.fields.shortId")}
                  {...register("shortId")}
                />
              </>
            ) : null}
            <Disclosure title={t("common.advanced")} className="lg:col-span-4">
              <TextField
                label={t("panes.profiles.fields.echConfigList")}
                {...register("echConfigList")}
              />
              <div className="grid min-w-0 gap-1 lg:col-span-2">
                <Label
                  className="text-xs text-muted-foreground"
                  htmlFor="profile-pinned-cert"
                >
                  <span className="truncate">
                    {t("panes.profiles.fields.pinnedCert")}
                  </span>
                </Label>
                <Textarea
                  className="min-h-24 resize-y bg-card font-mono text-xs"
                  id="profile-pinned-cert"
                  {...register("cert")}
                />
              </div>
            </Disclosure>
          </>
        ) : null}
      </div>
    </Panel>
  );
}
