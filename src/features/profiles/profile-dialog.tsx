import { useEffect, useId, useRef } from "react";
import type * as React from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { Save, Server, ShieldCheck } from "lucide-react";
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
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
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
import type { ProfileListItem_Serialize } from "@/ipc/bindings";
import { cn } from "@/lib/utils";
import { GroupBuilder } from "@/features/groups";

import {
  CONFIG_TYPES,
  CORE_TYPE_OPTIONS,
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

export function ProfileDialog({ mode, onOpenChange, onSubmit, open, profile }: ProfileDialogProps) {
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
      <DialogContent className="max-h-[92vh] w-[min(96vw,68rem)] grid-rows-[auto,minmax(0,1fr),auto] overflow-hidden">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Server className="size-4" aria-hidden="true" />
            {mode === "edit" ? "Edit profile" : "Add profile"}
          </DialogTitle>
          <DialogDescription className="sr-only">
            Create or edit a persisted proxy profile using the generated profile IPC contract.
          </DialogDescription>
        </DialogHeader>

        <form className="min-h-0 overflow-y-auto pe-1" id="profile-form" onSubmit={(event) => void submit(event)}>
          <div className="grid gap-4">
            <Panel title="Profile">
              <div className="grid gap-3 lg:grid-cols-[14rem_1fr_8rem]">
                <SelectField
                  control={form.control}
                  label="Protocol"
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

                <TextField error={errors.Remarks?.message} label="Remarks" {...register("Remarks")} />
                <SelectField
                  control={form.control}
                  label="Core"
                  name="CoreType"
                  options={CORE_TYPE_OPTIONS}
                  parseValue={(value) => (value === "" ? null : Number(value))}
                />
              </div>

              <div className="grid gap-3 lg:grid-cols-[1fr_7rem_12rem]">
                <TextField error={errors.Address?.message} label={addressLabel(configType)} {...register("Address")} />
                <TextField
                  error={errors.Port?.message}
                  inputMode="numeric"
                  label="Port"
                  type="number"
                  {...register("Port", { valueAsNumber: true })}
                />
                <TextField label="Group" {...register("Subid")} />
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
            <SecurityPanel control={form.control} register={register} security={security} />
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
            Cancel
          </Button>
          <Button disabled={isSubmitting} form="profile-form" type="submit">
            <Save className="size-4" aria-hidden="true" />
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
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
  if (configType === CONFIG_TYPES.PolicyGroup || configType === CONFIG_TYPES.ProxyChain) {
    return (
      <Panel title={configType === CONFIG_TYPES.PolicyGroup ? "Policy group" : "Proxy chain"}>
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
      <Panel title="Custom profile">
        <div className="grid gap-3 lg:grid-cols-2">
          <TextField label="Config source" {...register("Address")} />
          <TextField label="Filter" {...register("ProtocolExtra.Filter")} />
        </div>
      </Panel>
    );
  }

  return (
    <Panel title="Protocol">
      <div className="grid gap-3 lg:grid-cols-3">
        {requiresUsername(configType) ? <TextField label="Username" {...register("Username")} /> : null}
        <TextField label={passwordLabel(configType)} {...register("Password")} />
        {configType === CONFIG_TYPES.VMess ? (
          <>
            <TextField label="Alter ID" {...register("ProtocolExtra.AlterId")} />
            <TextField label="VMess security" placeholder="auto" {...register("ProtocolExtra.VmessSecurity")} />
          </>
        ) : null}
        {configType === CONFIG_TYPES.VLESS ? (
          <>
            <TextField label="Flow" placeholder="xtls-rprx-vision" {...register("ProtocolExtra.Flow")} />
            <TextField label="Encryption" placeholder="none" {...register("ProtocolExtra.VlessEncryption")} />
          </>
        ) : null}
        {configType === CONFIG_TYPES.Shadowsocks ? (
          <>
            <TextField label="Method" placeholder="2022-blake3-aes-128-gcm" {...register("ProtocolExtra.SsMethod")} />
            <CheckboxField control={control} label="UDP over TCP" name="ProtocolExtra.Uot" />
          </>
        ) : null}
        {configType === CONFIG_TYPES.Hysteria2 ? (
          <>
            <TextField
              inputMode="numeric"
              label="Up Mbps"
              type="number"
              {...register("ProtocolExtra.UpMbps", { setValueAs: optionalNumber })}
            />
            <TextField
              inputMode="numeric"
              label="Down Mbps"
              type="number"
              {...register("ProtocolExtra.DownMbps", { setValueAs: optionalNumber })}
            />
            <TextField label="Ports" {...register("ProtocolExtra.Ports")} />
            <TextField label="Hop interval" {...register("ProtocolExtra.HopInterval")} />
            <TextField label="Salamander password" {...register("ProtocolExtra.SalamanderPass")} />
          </>
        ) : null}
        {configType === CONFIG_TYPES.TUIC ? (
          <>
            <TextField label="Congestion control" placeholder="bbr" {...register("ProtocolExtra.CongestionControl")} />
            <TextField
              inputMode="numeric"
              label="Insecure concurrency"
              type="number"
              {...register("ProtocolExtra.InsecureConcurrency", { setValueAs: optionalNumber })}
            />
          </>
        ) : null}
        {configType === CONFIG_TYPES.WireGuard ? (
          <>
            <TextField label="Peer public key" {...register("ProtocolExtra.WgPublicKey")} />
            <TextField label="Preshared key" {...register("ProtocolExtra.WgPresharedKey")} />
            <TextField label="Interface address" {...register("ProtocolExtra.WgInterfaceAddress")} />
            <TextField label="Allowed IPs" {...register("ProtocolExtra.WgAllowedIps")} />
            <TextField label="Reserved bytes" {...register("ProtocolExtra.WgReserved")} />
            <TextField
              inputMode="numeric"
              label="MTU"
              type="number"
              {...register("ProtocolExtra.WgMtu", { setValueAs: optionalNumber })}
            />
          </>
        ) : null}
        {configType === CONFIG_TYPES.Naive ? (
          <CheckboxField control={control} label="QUIC" name="ProtocolExtra.NaiveQuic" />
        ) : null}
      </div>
    </Panel>
  );
}

function TransportPanel({ control, register }: { control: ProfileFormControl; register: Register }) {
  return (
    <Panel title="Transport">
      <div className="grid gap-3 lg:grid-cols-4">
        <SelectField control={control} label="Network" name="Network" options={NETWORK_OPTIONS} />
        <TextField label="Host" {...register("TransportExtra.Host")} />
        <TextField label="Path" {...register("TransportExtra.Path")} />
        <TextField label="Raw header" placeholder="none" {...register("TransportExtra.RawHeaderType")} />
        <TextField label="XHTTP mode" {...register("TransportExtra.XhttpMode")} />
        <TextField label="XHTTP extra JSON" {...register("TransportExtra.XhttpExtra")} />
        <TextField label="gRPC authority" {...register("TransportExtra.GrpcAuthority")} />
        <TextField label="gRPC service" {...register("TransportExtra.GrpcServiceName")} />
        <TextField label="gRPC mode" {...register("TransportExtra.GrpcMode")} />
        <TextField label="KCP header" {...register("TransportExtra.KcpHeaderType")} />
        <TextField label="KCP seed" {...register("TransportExtra.KcpSeed")} />
        <TextField
          inputMode="numeric"
          label="KCP MTU"
          type="number"
          {...register("TransportExtra.KcpMtu", { setValueAs: optionalNumber })}
        />
      </div>
    </Panel>
  );
}

function SecurityPanel({
  control,
  register,
  security,
}: {
  control: ProfileFormControl;
  register: Register;
  security: string;
}) {
  const reality = security === "reality";

  return (
    <Panel title="Security">
      <div className="grid gap-3 lg:grid-cols-4">
        <SelectField control={control} label="TLS mode" name="StreamSecurity" options={SECURITY_OPTIONS} />
        <TextField label="SNI" {...register("Sni")} />
        <TextField label="ALPN" {...register("Alpn")} />
        <TextField label="Fingerprint" {...register("Fingerprint")} />
        <TextField label={reality ? "REALITY public key" : "Public key"} {...register("PublicKey")} />
        <TextField label="Short ID" {...register("ShortId")} />
        <TextField label="Spider X" {...register("SpiderX")} />
        <TextField label="ML-DSA verify" {...register("Mldsa65Verify")} />
        <TextField label="ECH config list" {...register("EchConfigList")} />
        <TextField label="Final mask" {...register("Finalmask")} />
        <TextField label="Pinned cert" {...register("Cert")} />
        <TextField label="Cert SHA" {...register("CertSha")} />
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
  return (
    <Panel title="Mux">
      <div className="grid gap-3 lg:grid-cols-4">
        <ToggleButton
          checked={muxEnabled}
          description="Mux is stored per profile and interpreted by the core generator."
          label="Mux enabled"
          onCheckedChange={setMuxEnabled}
        />
        <ToggleButton
          checked={allowInsecure}
          description="Stored as the profile AllowInsecure string expected by the Rust DTO."
          label="Allow insecure TLS"
          onCheckedChange={setAllowInsecure}
        />
        <CheckboxField control={control} label="Display log" name="DisplayLog" />
      </div>
    </Panel>
  );
}

function Panel({ children, title }: { children: React.ReactNode; title: string }) {
  return (
    <Card className="gap-3 rounded-md bg-background p-3 shadow-none">
      <CardHeader className="p-0">
        <CardTitle className="flex items-center gap-2 text-sm">
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
            {checked ? "On" : "Off"}
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

function addressLabel(configType: ProfileProtocol) {
  if (configType === CONFIG_TYPES.Custom) {
    return "Config path / JSON";
  }
  if (configType === CONFIG_TYPES.PolicyGroup) {
    return "Group tag";
  }
  if (configType === CONFIG_TYPES.ProxyChain) {
    return "Chain tag";
  }

  return "Address";
}

function passwordLabel(configType: ProfileProtocol) {
  if (
    configType === CONFIG_TYPES.VMess ||
    configType === CONFIG_TYPES.VLESS ||
    configType === CONFIG_TYPES.TUIC
  ) {
    return "UUID";
  }
  if (configType === CONFIG_TYPES.WireGuard) {
    return "Private key";
  }

  return "Password";
}

function requiresUsername(configType: ProfileProtocol) {
  return configType === CONFIG_TYPES.SOCKS || configType === CONFIG_TYPES.HTTP || configType === CONFIG_TYPES.Naive;
}
