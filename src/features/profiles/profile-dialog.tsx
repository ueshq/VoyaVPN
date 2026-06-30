import { useEffect, useId, useRef, useState } from "react";
import type * as React from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { Download, Hash, Layers, Save, Server, ShieldCheck } from "lucide-react";
import { Controller, useForm, useWatch } from "react-hook-form";
import type {
  Control,
  FieldPath,
  UseFormGetValues,
  UseFormRegister,
  UseFormSetValue,
} from "react-hook-form";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import type { ProfileListItem_Serialize } from "@/ipc/bindings";
import { useI18n } from "@/i18n/use-i18n";
import { calculateCertificateSha256, fetchCertificate } from "@/ipc";
import { cn, getErrorMessage } from "@/lib/utils";
import { GroupBuilder } from "@/features/groups";

import {
  CONFIG_TYPES,
  NETWORK_OPTIONS,
  PROFILE_PROTOCOLS,
  SECURITY_OPTIONS,
  type ProfileProtocol,
} from "./profile-constants";
import {
  createDefaultProfile,
  normalizeProfileForForm,
  prepareProfileForSave,
  profileFormSchema,
  type ParsedProfileFormValues,
  type ProfileFormValues,
} from "./profile-form-schema";

type ProfileDialogProps = {
  mode: "create" | "edit";
  onOpenChange: (open: boolean) => void;
  onSubmit: (profile: ReturnType<typeof prepareProfileForSave>) => Promise<void>;
  open: boolean;
  profile?: ProfileListItem_Serialize | null;
};

type Translate = (key: string, options?: Record<string, unknown>) => string;

export function ProfileDialog({ mode, onOpenChange, onSubmit, open, profile }: ProfileDialogProps) {
  const { t } = useI18n();
  const form = useForm<ProfileFormValues, unknown, ParsedProfileFormValues>({
    defaultValues: profile ? normalizeProfileForForm(profile.profile) : createDefaultProfile(),
    mode: "onBlur",
    resolver: zodResolver(profileFormSchema),
  });
  const {
    formState: { errors, isSubmitting },
    getValues,
    handleSubmit,
    register,
    reset,
    setValue,
  } = form;
  const configType = Number(useWatch({ control: form.control, name: "ConfigType" })) as ProfileProtocol;
  const security = useWatch({ control: form.control, name: "StreamSecurity" }) ?? "";
  const allowInsecure = useWatch({ control: form.control, name: "AllowInsecure" }) === "true";
  const muxEnabled = useWatch({ control: form.control, name: "MuxEnabled" }) === true;
  const resetKeyRef = useRef<string | null>(null);

  useEffect(() => {
    if (!open) {
      resetKeyRef.current = null;
      return;
    }

    const resetKey = `${mode}:${profile?.profile.IndexId ?? "new"}`;
    if (resetKeyRef.current === resetKey) {
      return;
    }

    resetKeyRef.current = resetKey;
    reset(profile ? normalizeProfileForForm(profile.profile) : createDefaultProfile());
  }, [mode, profile, reset, open]);

  const submit = handleSubmit(async (values) => {
    await onSubmit(prepareProfileForSave(values));
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <ScrollableDialogContent width="68rem">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Server className="size-4" aria-hidden="true" />
            {mode === "edit" ? t("panes.profiles.dialog.editTitle") : t("panes.profiles.dialog.addTitle")}
          </DialogTitle>
          <DialogDescription className="sr-only">
            {t("panes.profiles.dialog.description")}
          </DialogDescription>
        </DialogHeader>

        <form className="min-h-0 overflow-y-auto pe-1" id="profile-form" onSubmit={(event) => void submit(event)}>
          <div className="grid gap-4">
            <Panel title={t("panes.profiles.panels.profile")}>
              <div className="grid gap-3 lg:grid-cols-[14rem_1fr]">
                <SelectField
                  control={form.control}
                  label={t("panes.profiles.fields.protocol")}
                  name="ConfigType"
                  onValueChange={(value) => {
                    const next = Number(value) as ProfileProtocol;

                    if (next === CONFIG_TYPES.PolicyGroup && !getValues("Address")) {
                      setValue("Address", "group");
                    }
                    if (next === CONFIG_TYPES.ProxyChain && !getValues("Address")) {
                      setValue("Address", "chain");
                    }
                  }}
                  options={PROFILE_PROTOCOLS}
                  parseValue={(value) => Number(value)}
                />

                <TextField error={errors.Remarks?.message} label={t("panes.profiles.fields.remarks")} {...register("Remarks")} />
              </div>

              <div className="grid gap-3 lg:grid-cols-[1fr_7rem_12rem]">
                <TextField error={errors.Address?.message} label={addressLabel(configType, t)} {...register("Address")} />
                <TextField
                  error={errors.Port?.message}
                  inputMode="numeric"
                  label={t("panes.profiles.fields.port")}
                  type="number"
                  {...register("Port", { valueAsNumber: true })}
                />
                <TextField label={t("panes.profiles.fields.group")} {...register("Subid")} />
              </div>
            </Panel>

            <ProtocolPanel
              configType={configType}
              control={form.control}
              getValues={getValues}
              register={register}
              setValue={setValue}
            />
            <TransportPanel control={form.control} register={register} />
            <SecurityPanel
              control={form.control}
              getValues={getValues}
              register={register}
              security={security}
              setValue={setValue}
            />
            <MuxPanel
              allowInsecure={allowInsecure}
              control={form.control}
              muxEnabled={muxEnabled}
              setAllowInsecure={(enabled) => setValue("AllowInsecure", enabled ? "true" : "false")}
              setMuxEnabled={(enabled) => setValue("MuxEnabled", enabled)}
            />
          </div>
        </form>

        <DialogFooter>
          <Button disabled={isSubmitting} onClick={() => onOpenChange(false)} type="button" variant="outline">
            {t("panes.profiles.dialog.cancel")}
          </Button>
          <Button disabled={isSubmitting} form="profile-form" type="submit">
            <Save className="size-4" aria-hidden="true" />
            {t("panes.profiles.dialog.save")}
          </Button>
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}

type Register = UseFormRegister<ProfileFormValues>;
type ProfileFormControl = Control<ProfileFormValues, unknown, ParsedProfileFormValues>;

function ProtocolPanel({
  configType,
  control,
  getValues,
  register,
  setValue,
}: {
  configType: ProfileProtocol;
  control: ProfileFormControl;
  getValues: UseFormGetValues<ProfileFormValues>;
  register: Register;
  setValue: UseFormSetValue<ProfileFormValues>;
}) {
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
    return (
      <Panel title={t("panes.profiles.panels.custom")}>
        <div className="grid gap-3 lg:grid-cols-2">
          <TextField label={t("panes.profiles.fields.configSource")} {...register("Address")} />
          <TextField label={t("panes.profiles.fields.filter")} {...register("ProtocolExtra.Filter")} />
        </div>
      </Panel>
    );
  }

  return (
    <Panel title={t("panes.profiles.panels.protocol")}>
      <div className="grid gap-3 lg:grid-cols-3">
        {requiresUsername(configType) ? (
          <TextField label={t("panes.profiles.fields.username")} {...register("Username")} />
        ) : null}
        <TextField label={passwordLabel(configType, t)} {...register("Password")} />
        {configType === CONFIG_TYPES.VMess ? (
          <>
            <TextField label={t("panes.profiles.fields.alterId")} {...register("ProtocolExtra.AlterId")} />
            <TextField label={t("panes.profiles.fields.vmessSecurity")} placeholder="auto" {...register("ProtocolExtra.VmessSecurity")} />
          </>
        ) : null}
        {configType === CONFIG_TYPES.VLESS ? (
          <>
            <TextField label={t("panes.profiles.fields.flow")} placeholder="xtls-rprx-vision" {...register("ProtocolExtra.Flow")} />
            <TextField label={t("panes.profiles.fields.encryption")} placeholder="none" {...register("ProtocolExtra.VlessEncryption")} />
          </>
        ) : null}
        {configType === CONFIG_TYPES.Shadowsocks ? (
          <>
            <TextField label={t("panes.profiles.fields.method")} placeholder="2022-blake3-aes-128-gcm" {...register("ProtocolExtra.SsMethod")} />
            <CheckboxField control={control} label={t("panes.profiles.fields.udpOverTcp")} name="ProtocolExtra.Uot" />
          </>
        ) : null}
        {configType === CONFIG_TYPES.Hysteria2 ? (
          <>
            <TextField
              inputMode="numeric"
              label={t("panes.profiles.fields.upMbps")}
              type="number"
              {...register("ProtocolExtra.UpMbps", { setValueAs: optionalNumber })}
            />
            <TextField
              inputMode="numeric"
              label={t("panes.profiles.fields.downMbps")}
              type="number"
              {...register("ProtocolExtra.DownMbps", { setValueAs: optionalNumber })}
            />
            <TextField label={t("panes.profiles.fields.ports")} {...register("ProtocolExtra.Ports")} />
            <TextField label={t("panes.profiles.fields.hopInterval")} {...register("ProtocolExtra.HopInterval")} />
            <TextField label={t("panes.profiles.fields.salamanderPassword")} {...register("ProtocolExtra.SalamanderPass")} />
          </>
        ) : null}
        {configType === CONFIG_TYPES.TUIC ? (
          <>
            <TextField label={t("panes.profiles.fields.congestionControl")} placeholder="bbr" {...register("ProtocolExtra.CongestionControl")} />
            <TextField
              inputMode="numeric"
              label={t("panes.profiles.fields.insecureConcurrency")}
              type="number"
              {...register("ProtocolExtra.InsecureConcurrency", { setValueAs: optionalNumber })}
            />
          </>
        ) : null}
        {configType === CONFIG_TYPES.WireGuard ? (
          <>
            <TextField label={t("panes.profiles.fields.peerPublicKey")} {...register("ProtocolExtra.WgPublicKey")} />
            <TextField label={t("panes.profiles.fields.presharedKey")} {...register("ProtocolExtra.WgPresharedKey")} />
            <TextField label={t("panes.profiles.fields.interfaceAddress")} {...register("ProtocolExtra.WgInterfaceAddress")} />
            <TextField label={t("panes.profiles.fields.allowedIps")} {...register("ProtocolExtra.WgAllowedIps")} />
            <TextField label={t("panes.profiles.fields.reservedBytes")} {...register("ProtocolExtra.WgReserved")} />
            <TextField
              inputMode="numeric"
              label={t("panes.profiles.fields.mtu")}
              type="number"
              {...register("ProtocolExtra.WgMtu", { setValueAs: optionalNumber })}
            />
          </>
        ) : null}
        {configType === CONFIG_TYPES.Naive ? (
          <CheckboxField control={control} label={t("panes.profiles.fields.quic")} name="ProtocolExtra.NaiveQuic" />
        ) : null}
      </div>
    </Panel>
  );
}

function TransportPanel({ control, register }: { control: ProfileFormControl; register: Register }) {
  const { t } = useI18n();

  return (
    <Panel title={t("panes.profiles.panels.transport")}>
      <div className="grid gap-3 lg:grid-cols-4">
        <SelectField control={control} label={t("panes.profiles.fields.network")} name="Network" options={NETWORK_OPTIONS} />
        <TextField label={t("panes.profiles.fields.host")} {...register("TransportExtra.Host")} />
        <TextField label={t("panes.profiles.fields.path")} {...register("TransportExtra.Path")} />
        <TextField label={t("panes.profiles.fields.rawHeader")} placeholder="none" {...register("TransportExtra.RawHeaderType")} />
        <TextField label={t("panes.profiles.fields.xhttpMode")} {...register("TransportExtra.XhttpMode")} />
        <TextField label={t("panes.profiles.fields.xhttpExtra")} {...register("TransportExtra.XhttpExtra")} />
        <TextField label={t("panes.profiles.fields.grpcAuthority")} {...register("TransportExtra.GrpcAuthority")} />
        <TextField label={t("panes.profiles.fields.grpcService")} {...register("TransportExtra.GrpcServiceName")} />
        <TextField label={t("panes.profiles.fields.grpcMode")} {...register("TransportExtra.GrpcMode")} />
        <TextField label={t("panes.profiles.fields.kcpHeader")} {...register("TransportExtra.KcpHeaderType")} />
        <TextField label={t("panes.profiles.fields.kcpSeed")} {...register("TransportExtra.KcpSeed")} />
        <TextField
          inputMode="numeric"
          label={t("panes.profiles.fields.kcpMtu")}
          type="number"
          {...register("TransportExtra.KcpMtu", { setValueAs: optionalNumber })}
        />
      </div>
    </Panel>
  );
}

function SecurityPanel({
  control,
  getValues,
  register,
  security,
  setValue,
}: {
  control: ProfileFormControl;
  getValues: UseFormGetValues<ProfileFormValues>;
  register: Register;
  security: string;
  setValue: UseFormSetValue<ProfileFormValues>;
}) {
  const { t } = useI18n();
  const [allowInsecureFetch, setAllowInsecureFetch] = useState(false);
  const [certError, setCertError] = useState<string | null>(null);
  const [certStatus, setCertStatus] = useState<string | null>(null);
  const [certWorking, setCertWorking] = useState(false);
  const reality = security === "reality";

  async function fetchRemoteCertificate(includeChain: boolean) {
    const address = String(getValues("Address") ?? "").trim();
    const port = Number(getValues("Port") ?? 0);
    const serverName = String(getValues("Sni") || address).trim();
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
      setValue("Cert", result.pem, { shouldDirty: true, shouldTouch: true, shouldValidate: true });
      setValue("CertSha", result.sha256.join(","), { shouldDirty: true, shouldTouch: true, shouldValidate: true });
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
    const pem = String(getValues("Cert") ?? "").trim();
    if (!pem) {
      setCertError(t("panes.profiles.certFetch.missingPem"));
      return;
    }

    setCertWorking(true);
    setCertError(null);
    setCertStatus(null);
    try {
      const hashes = await calculateCertificateSha256(pem);
      setValue("CertSha", hashes.join(","), { shouldDirty: true, shouldTouch: true, shouldValidate: true });
      setCertStatus(t("panes.profiles.certFetch.shaCalculated", { count: hashes.length }));
    } catch (error) {
      setCertError(getErrorMessage(error));
    } finally {
      setCertWorking(false);
    }
  }

  return (
    <Panel title={t("panes.profiles.panels.security")}>
      <div className="grid gap-3 lg:grid-cols-4">
        <SelectField control={control} label={t("panes.profiles.fields.tlsMode")} name="StreamSecurity" options={SECURITY_OPTIONS} />
        <TextField label={t("panes.profiles.fields.sni")} {...register("Sni")} />
        <TextField label={t("panes.profiles.fields.alpn")} {...register("Alpn")} />
        <TextField label={t("panes.profiles.fields.fingerprint")} {...register("Fingerprint")} />
        <TextField
          label={reality ? t("panes.profiles.fields.realityPublicKey") : t("panes.profiles.fields.publicKey")}
          {...register("PublicKey")}
        />
        <TextField label={t("panes.profiles.fields.shortId")} {...register("ShortId")} />
        <TextField label={t("panes.profiles.fields.spiderX")} {...register("SpiderX")} />
        <TextField label={t("panes.profiles.fields.mldsaVerify")} {...register("Mldsa65Verify")} />
        <TextField label={t("panes.profiles.fields.echConfigList")} {...register("EchConfigList")} />
        <TextField label={t("panes.profiles.fields.finalMask")} {...register("Finalmask")} />
        <div className="grid min-w-0 gap-1 lg:col-span-2">
          <Label className="text-xs text-muted-foreground" htmlFor="profile-pinned-cert">
            <span className="truncate">{t("panes.profiles.fields.pinnedCert")}</span>
          </Label>
          <Textarea
            className="min-h-24 resize-y bg-card font-mono text-xs"
            id="profile-pinned-cert"
            {...register("Cert")}
          />
        </div>
        <TextField className="font-mono text-xs" label={t("panes.profiles.fields.certSha")} {...register("CertSha")} />
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
          {certStatus ? <p className="text-xs text-muted-foreground">{certStatus}</p> : null}
          {certError ? <p className="text-xs text-destructive">{certError}</p> : null}
        </div>
      </div>
    </Panel>
  );
}

function MuxPanel({
  allowInsecure,
  control,
  muxEnabled,
  setAllowInsecure,
  setMuxEnabled,
}: {
  allowInsecure: boolean;
  control: ProfileFormControl;
  muxEnabled: boolean;
  setAllowInsecure: (enabled: boolean) => void;
  setMuxEnabled: (enabled: boolean) => void;
}) {
  const { t } = useI18n();

  return (
    <Panel title={t("panes.profiles.panels.mux")}>
      <div className="grid gap-3 lg:grid-cols-4">
        <ToggleButton
          checked={muxEnabled}
          description={t("panes.profiles.fields.muxEnabledHint")}
          label={t("panes.profiles.fields.muxEnabled")}
          onCheckedChange={setMuxEnabled}
        />
        <ToggleButton
          checked={allowInsecure}
          description={t("panes.profiles.fields.allowInsecureTlsHint")}
          label={t("panes.profiles.fields.allowInsecureTls")}
          onCheckedChange={setAllowInsecure}
        />
        <CheckboxField control={control} label={t("panes.profiles.fields.displayLog")} name="DisplayLog" />
      </div>
    </Panel>
  );
}

function Panel({ children, title }: { children: React.ReactNode; title: string }) {
  return (
    <Card className="gap-3 rounded-xl bg-surface-raised p-3 shadow-raised">
      <CardHeader className="p-0">
        <CardTitle className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground">
          <ShieldCheck className="size-4 text-muted-foreground" aria-hidden="true" />
          {title}
        </CardTitle>
      </CardHeader>
      <CardContent className="p-0">{children}</CardContent>
    </Card>
  );
}

type TextFieldProps = React.InputHTMLAttributes<HTMLInputElement> & {
  error?: string;
  label: string;
};

const TextField = ({ className, error, id, label, ...props }: TextFieldProps) => {
  const inputId = id ?? fieldId(label);
  const errorId = `${inputId}-error`;
  const {
    "aria-describedby": ariaDescribedBy,
    "aria-invalid": ariaInvalid,
    ...inputProps
  } = props;

  return (
    <div className="grid min-w-0 gap-1">
      <Label className="text-xs text-muted-foreground" htmlFor={inputId}>
        <span className="truncate">{label}</span>
      </Label>
      <Input
        aria-describedby={error ? mergeIds(ariaDescribedBy, errorId) : ariaDescribedBy}
        aria-invalid={error ? true : ariaInvalid}
        className={cn("bg-card", className)}
        id={inputId}
        {...inputProps}
      />
      {error ? (
        <span className="text-xs text-destructive" id={errorId}>
          {error}
        </span>
      ) : null}
    </div>
  );
};

type SelectOption = {
  description?: string;
  label: string;
  value: number | string;
};

type SelectFieldProps = {
  className?: string;
  control: ProfileFormControl;
  error?: string;
  id?: string;
  label: string;
  name: FieldPath<ProfileFormValues>;
  onValueChange?: (value: string) => void;
  options: SelectOption[];
  parseValue?: (value: string) => unknown;
};

const SelectField = ({
  className,
  control,
  error,
  id,
  label,
  name,
  onValueChange,
  options,
  parseValue,
}: SelectFieldProps) => {
  const inputId = id ?? fieldId(label);
  const errorId = `${inputId}-error`;

  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => {
        const invalid = Boolean(error ?? fieldState.error?.message);

        return (
          <div className="grid min-w-0 gap-1">
            <Label className="text-xs text-muted-foreground" htmlFor={inputId}>
              <span className="truncate">{label}</span>
            </Label>
            <Select
              name={field.name}
              onValueChange={(value) => {
                const decoded = decodeSelectValue(value);

                field.onChange(parseValue ? parseValue(decoded) : decoded);
                onValueChange?.(decoded);
              }}
              value={encodeSelectValue(field.value)}
            >
              <SelectTrigger
                aria-describedby={error ? errorId : undefined}
                aria-invalid={invalid ? true : undefined}
                className={cn("w-full bg-card", className)}
                id={inputId}
                onBlur={field.onBlur}
                ref={field.ref}
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {options.map((option) => (
                  <SelectItem key={`${name}-${option.value}`} value={encodeSelectValue(option.value)}>
                    <span>{option.label}</span>
                    {option.description ? <span className="sr-only">{option.description}</span> : null}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {error ? (
              <span className="text-xs text-destructive" id={errorId}>
                {error}
              </span>
            ) : null}
          </div>
        );
      }}
    />
  );
};

type CheckboxFieldProps = {
  className?: string;
  control: ProfileFormControl;
  id?: string;
  label: string;
  name: FieldPath<ProfileFormValues>;
};

const CheckboxField = ({ className, control, id, label, name }: CheckboxFieldProps) => {
  const inputId = id ?? fieldId(label);

  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <Card className={cn("h-9 min-w-0 justify-center gap-0 rounded-md bg-card px-3 py-0 shadow-none", className)}>
          <Label
            className="h-full w-full min-w-0 cursor-pointer text-xs font-medium text-muted-foreground"
            htmlFor={inputId}
          >
            <Checkbox
              aria-invalid={fieldState.invalid ? true : undefined}
              checked={field.value === true}
              id={inputId}
              name={field.name}
              onBlur={field.onBlur}
              onCheckedChange={(checked) => field.onChange(checked === true)}
              ref={field.ref}
            />
            <span className="truncate">{label}</span>
          </Label>
        </Card>
      )}
    />
  );
};

function ToggleButton({
  checked,
  description,
  label,
  onCheckedChange,
}: {
  checked: boolean;
  description: string;
  label: string;
  onCheckedChange: (enabled: boolean) => void;
}) {
  const { t } = useI18n();
  const generatedId = useId();
  const inputId = `${fieldId(label)}-${generatedId}`;

  return (
    <Card
      className={cn(
        "h-16 justify-center gap-0 rounded-md px-3 py-0 shadow-none transition-colors",
        checked ? "border-primary bg-accent/60" : "bg-card",
      )}
      title={description}
    >
      <Label className="h-full w-full min-w-0 cursor-pointer justify-between gap-3 text-xs" htmlFor={inputId}>
        <span className="grid min-w-0 gap-1">
          <span className="truncate font-medium text-foreground">{label}</span>
          <Badge className="w-fit" variant={checked ? "default" : "secondary"}>
            {checked ? t("panes.profiles.toggle.on") : t("panes.profiles.toggle.off")}
          </Badge>
        </span>
        <Switch aria-label={label} checked={checked} id={inputId} onCheckedChange={onCheckedChange} />
      </Label>
    </Card>
  );
}

function optionalNumber(value: unknown) {
  if (value === "" || value === null || value === undefined) {
    return null;
  }

  return Number(value);
}

function fieldId(label: string) {
  return label.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-").replaceAll(/^-|-$/g, "");
}

const EMPTY_SELECT_VALUE = "__voyavpn_empty__";

function encodeSelectValue(value: unknown) {
  const stringValue = value === null || value === undefined ? "" : String(value);

  return stringValue === "" ? EMPTY_SELECT_VALUE : stringValue;
}

function decodeSelectValue(value: string) {
  return value === EMPTY_SELECT_VALUE ? "" : value;
}

function mergeIds(...ids: Array<string | undefined>) {
  return ids.filter(Boolean).join(" ") || undefined;
}

function addressLabel(configType: ProfileProtocol, t: Translate) {
  if (configType === CONFIG_TYPES.Custom) {
    return t("panes.profiles.fields.addressConfig");
  }
  if (configType === CONFIG_TYPES.PolicyGroup) {
    return t("panes.profiles.fields.addressGroupTag");
  }
  if (configType === CONFIG_TYPES.ProxyChain) {
    return t("panes.profiles.fields.addressChainTag");
  }

  return t("panes.profiles.fields.address");
}

function passwordLabel(configType: ProfileProtocol, t: Translate) {
  if (
    configType === CONFIG_TYPES.VMess ||
    configType === CONFIG_TYPES.VLESS ||
    configType === CONFIG_TYPES.TUIC
  ) {
    return t("panes.profiles.fields.uuid");
  }
  if (configType === CONFIG_TYPES.WireGuard) {
    return t("panes.profiles.fields.privateKey");
  }

  return t("panes.profiles.fields.password");
}

function requiresUsername(configType: ProfileProtocol) {
  return configType === CONFIG_TYPES.SOCKS || configType === CONFIG_TYPES.HTTP || configType === CONFIG_TYPES.Naive;
}
