import { useState } from "react";
import { Download, Hash, Layers } from "lucide-react";
import type { UseFormGetValues, UseFormSetValue } from "react-hook-form";

import { Disclosure } from "@voya/ui/components/disclosure";
import { Button } from "@voya/ui/components/button";
import { Label } from "@voya/ui/components/label";
import { Switch } from "@voya/ui/components/switch";
import { Textarea } from "@voya/ui/components/textarea";
import { useI18n } from "@voya/i18n/use-i18n";
import { calculateCertificateSha256, fetchCertificate } from "@/ipc";
import { getErrorMessage } from "@voya/utils/error";

import { SECURITY_OPTIONS } from "./profile-constants";
import {
  Panel,
  SelectField,
  TextField,
  type ProfileFormControl,
  type Register,
} from "./profile-form-fields";
import type { ProfileFormValues } from "./profile-form-schema";

type SecurityPanelProps = {
  control: ProfileFormControl;
  getValues: UseFormGetValues<ProfileFormValues>;
  register: Register;
  security: string;
  setValue: UseFormSetValue<ProfileFormValues>;
};

export function SecurityPanel({
  control,
  getValues,
  register,
  security,
  setValue,
}: SecurityPanelProps) {
  const { t } = useI18n();
  const [allowInsecureFetch, setAllowInsecureFetch] = useState(false);
  const [certError, setCertError] = useState<string | null>(null);
  const [certStatus, setCertStatus] = useState<string | null>(null);
  const [certWorking, setCertWorking] = useState(false);
  const reality = security === "reality";

  async function fetchRemoteCertificate(includeChain: boolean) {
    const address = String(getValues("address") ?? "").trim();
    const port = Number(getValues("port") ?? 0);
    const serverName = String(getValues("sni") || address).trim();
    if (!address || !Number.isFinite(port) || port <= 0) {
      setCertError(t("panes.profiles.certFetch.missingEndpoint"));
      return;
    }

    setCertWorking(true);
    setCertError(null);
    setCertStatus(null);
    try {
      const result = await fetchCertificate({
        address,
        allowInsecure: allowInsecureFetch,
        includeChain,
        port,
        serverName: serverName || null,
      });
      setValue("cert", result.pem, {
        shouldDirty: true,
        shouldTouch: true,
        shouldValidate: true,
      });
      setValue("certSha", result.sha256.join(","), {
        shouldDirty: true,
        shouldTouch: true,
        shouldValidate: true,
      });
      setCertStatus(
        result.warning ||
          t("panes.profiles.certFetch.fetched", { count: result.chainCount }),
      );
    } catch (error) {
      setCertError(getErrorMessage(error));
    } finally {
      setCertWorking(false);
    }
  }

  async function calculatePinnedCertificateSha() {
    const pem = String(getValues("cert") ?? "").trim();
    if (!pem) {
      setCertError(t("panes.profiles.certFetch.missingPem"));
      return;
    }

    setCertWorking(true);
    setCertError(null);
    setCertStatus(null);
    try {
      const hashes = await calculateCertificateSha256(pem);
      setValue("certSha", hashes.join(","), {
        shouldDirty: true,
        shouldTouch: true,
        shouldValidate: true,
      });
      setCertStatus(
        t("panes.profiles.certFetch.shaCalculated", { count: hashes.length }),
      );
    } catch (error) {
      setCertError(getErrorMessage(error));
    } finally {
      setCertWorking(false);
    }
  }

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
                  label={
                    reality
                      ? t("panes.profiles.fields.realityPublicKey")
                      : t("panes.profiles.fields.publicKey")
                  }
                  {...register("publicKey")}
                />
                <TextField
                  label={t("panes.profiles.fields.shortId")}
                  {...register("shortId")}
                />
                <TextField
                  label={t("panes.profiles.fields.spiderX")}
                  {...register("spiderX")}
                />
                <TextField
                  label={t("panes.profiles.fields.mldsaVerify")}
                  {...register("mldsa65Verify")}
                />
              </>
            ) : null}
            <Disclosure title={t("common.advanced")} className="lg:col-span-4">
              <TextField
                label={t("panes.profiles.fields.echConfigList")}
                {...register("echConfigList")}
              />
              <TextField
                label={t("panes.profiles.fields.finalMask")}
                {...register("finalmask")}
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
              <TextField
                className="font-mono text-xs"
                label={t("panes.profiles.fields.certSha")}
                {...register("certSha")}
              />
              <div className="grid gap-2 lg:col-span-4">
                <div className="flex flex-wrap items-center gap-2">
                  <Button
                    disabled={certWorking}
                    onClick={() => void fetchRemoteCertificate(false)}
                    type="button"
                    variant="outline"
                  >
                    <Download className="size-4" aria-hidden="true" />
                    {t("panes.profiles.certFetch.fetchCert")}
                  </Button>
                  <Button
                    disabled={certWorking}
                    onClick={() => void fetchRemoteCertificate(true)}
                    type="button"
                    variant="outline"
                  >
                    <Layers className="size-4" aria-hidden="true" />
                    {t("panes.profiles.certFetch.fetchChain")}
                  </Button>
                  <Button
                    disabled={certWorking}
                    onClick={() => void calculatePinnedCertificateSha()}
                    type="button"
                    variant="outline"
                  >
                    <Hash className="size-4" aria-hidden="true" />
                    {t("panes.profiles.certFetch.calculateSha")}
                  </Button>
                  <Label className="ms-auto flex min-h-9 cursor-pointer items-center gap-2 rounded-md border bg-card px-3 text-xs text-muted-foreground">
                    <Switch
                      aria-label={t("panes.profiles.certFetch.allowInsecure")}
                      checked={allowInsecureFetch}
                      onCheckedChange={setAllowInsecureFetch}
                    />
                    {t("panes.profiles.certFetch.allowInsecure")}
                  </Label>
                </div>
                {certStatus ? (
                  <p className="text-xs text-muted-foreground">{certStatus}</p>
                ) : null}
                {certError ? (
                  <p className="text-xs text-danger">{certError}</p>
                ) : null}
              </div>
            </Disclosure>
          </>
        ) : null}
      </div>
    </Panel>
  );
}
