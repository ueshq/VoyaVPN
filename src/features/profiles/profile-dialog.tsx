import { useEffect } from "react";
import type * as React from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { Save, Server, ShieldCheck } from "lucide-react";
import { useForm, useWatch } from "react-hook-form";
import type {
  Control,
  UseFormGetValues,
  UseFormRegister,
  UseFormSetValue,
} from "react-hook-form";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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

  useEffect(() => {
    reset(profile ? normalizeProfileForForm(profile.profile) : createDefaultProfile());
  }, [profile, reset, open]);

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
                  label="Protocol"
                  {...register("ConfigType", {
                    onChange: (event) => {
                      const next = Number(event.target.value) as ProfileProtocol;

                      if (next === CONFIG_TYPES.PolicyGroup && !getValues("Address")) {
                        setValue("Address", "group");
                      }
                      if (next === CONFIG_TYPES.ProxyChain && !getValues("Address")) {
                        setValue("Address", "chain");
                      }
                    },
                    valueAsNumber: true,
                  })}
                >
                  {PROFILE_PROTOCOLS.map((protocol) => (
                    <option key={protocol.value} value={protocol.value}>
                      {protocol.label}
                    </option>
                  ))}
                </SelectField>

                <TextField error={errors.Remarks?.message} label="Remarks" {...register("Remarks")} />
                <SelectField
                  label="Core"
                  {...register("CoreType", {
                    setValueAs: (value) => (value === "" ? null : Number(value)),
                  })}
                >
                  {CORE_TYPE_OPTIONS.map((core) => (
                    <option key={core.value} value={core.value}>
                      {core.label}
                    </option>
                  ))}
                </SelectField>
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
            <TransportPanel register={register} />
            <SecurityPanel register={register} security={security} />
            <MuxPanel
              allowInsecure={allowInsecure}
              muxEnabled={muxEnabled}
              register={register}
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

function ProtocolPanel({
  configType,
  control,
  getValues,
  register,
  setValue,
}: {
  configType: ProfileProtocol;
  control: Control<ProfileFormValues, unknown, ParsedProfileFormValues>;
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
            <CheckboxField label="UDP over TCP" {...register("ProtocolExtra.Uot")} />
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
            <TextField label="Reserved bytes" {...register("ProtocolExtra.WgReserved")} />
            <TextField
              inputMode="numeric"
              label="MTU"
              type="number"
              {...register("ProtocolExtra.WgMtu", { setValueAs: optionalNumber })}
            />
          </>
        ) : null}
        {configType === CONFIG_TYPES.Naive ? <CheckboxField label="QUIC" {...register("ProtocolExtra.NaiveQuic")} /> : null}
      </div>
    </Panel>
  );
}

function TransportPanel({ register }: { register: Register }) {
  return (
    <Panel title="Transport">
      <div className="grid gap-3 lg:grid-cols-4">
        <SelectField label="Network" {...register("Network")}>
          {NETWORK_OPTIONS.map((network) => (
            <option key={network.value} value={network.value}>
              {network.label}
            </option>
          ))}
        </SelectField>
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

function SecurityPanel({ register, security }: { register: Register; security: string }) {
  const reality = security === "reality";

  return (
    <Panel title="Security">
      <div className="grid gap-3 lg:grid-cols-4">
        <SelectField label="TLS mode" {...register("StreamSecurity")}>
          {SECURITY_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </SelectField>
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
  muxEnabled,
  register,
  setAllowInsecure,
  setMuxEnabled,
}: {
  allowInsecure: boolean;
  muxEnabled: boolean;
  register: Register;
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
        <CheckboxField label="Display log" {...register("DisplayLog")} />
      </div>
    </Panel>
  );
}

function Panel({ children, title }: { children: React.ReactNode; title: string }) {
  return (
    <section className="grid gap-3 rounded-md border bg-background p-3">
      <h3 className="flex items-center gap-2 text-sm font-semibold">
        <ShieldCheck className="size-4 text-muted-foreground" aria-hidden="true" />
        {title}
      </h3>
      {children}
    </section>
  );
}

type TextFieldProps = React.InputHTMLAttributes<HTMLInputElement> & {
  error?: string;
  label: string;
};

const TextField = ({ className, error, id, label, ...props }: TextFieldProps) => {
  const inputId = id ?? fieldId(label);

  return (
    <label className="grid min-w-0 gap-1 text-xs font-medium text-muted-foreground" htmlFor={inputId}>
      <span className="truncate">{label}</span>
      <input
        className={cn(
          "h-9 min-w-0 rounded-md border bg-card px-3 text-sm text-foreground outline-none transition-colors focus:border-ring focus:ring-2 focus:ring-ring/30",
          error ? "border-destructive" : null,
          className,
        )}
        id={inputId}
        {...props}
      />
      {error ? <span className="text-xs text-destructive">{error}</span> : null}
    </label>
  );
};

type SelectFieldProps = React.SelectHTMLAttributes<HTMLSelectElement> & {
  label: string;
};

const SelectField = ({ children, className, id, label, ...props }: SelectFieldProps) => {
  const inputId = id ?? fieldId(label);

  return (
    <label className="grid min-w-0 gap-1 text-xs font-medium text-muted-foreground" htmlFor={inputId}>
      <span className="truncate">{label}</span>
      <select
        className={cn(
          "h-9 min-w-0 rounded-md border bg-card px-2 text-sm text-foreground outline-none transition-colors focus:border-ring focus:ring-2 focus:ring-ring/30",
          className,
        )}
        id={inputId}
        {...props}
      >
        {children}
      </select>
    </label>
  );
};

type CheckboxFieldProps = Omit<React.InputHTMLAttributes<HTMLInputElement>, "type"> & {
  label: string;
};

const CheckboxField = ({ className, id, label, ...props }: CheckboxFieldProps) => {
  const inputId = id ?? fieldId(label);

  return (
    <label
      className={cn(
        "flex h-9 min-w-0 items-center gap-2 rounded-md border bg-card px-3 text-xs font-medium text-muted-foreground",
        className,
      )}
      htmlFor={inputId}
    >
      <input className="size-4 accent-primary" id={inputId} type="checkbox" {...props} />
      <span className="truncate">{label}</span>
    </label>
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
  return (
    <button
      aria-checked={checked}
      className={cn(
        "grid h-16 content-center gap-1 rounded-md border px-3 text-start text-xs transition-colors",
        checked ? "border-primary bg-accent text-accent-foreground" : "bg-card text-muted-foreground",
      )}
      onClick={() => onCheckedChange(!checked)}
      role="switch"
      title={description}
      type="button"
    >
      <span className="font-medium text-foreground">{label}</span>
      <span className="truncate">{checked ? "On" : "Off"}</span>
    </button>
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
