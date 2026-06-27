import { useEffect, useState } from "react";
import { Braces, Save, Settings2 } from "lucide-react";

import { Button } from "@/components/ui/button";
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { useI18n } from "@/i18n/use-i18n";
import { loadAppConfig, saveAppConfig } from "@/ipc";
import type { AppConfig_Serialize, CoreTypeItem, SysProxyType } from "@/ipc/bindings";
import { getErrorMessage } from "@/lib/utils";

type ObjectSectionKey =
  | "CheckUpdateItem"
  | "ConstItem"
  | "CoreBasicItem"
  | "Fragment4RayItem"
  | "HysteriaItem"
  | "Mux4RayItem"
  | "Mux4SboxItem"
  | "SpeedTestItem"
  | "SystemProxyItem"
  | "TunModeItem";

const sysProxyOptions: Array<{ label: string; value: SysProxyType }> = [
  { label: "Clear", value: 0 },
  { label: "Set", value: 1 },
  { label: "Unchanged", value: 2 },
  { label: "PAC", value: 3 },
];

export function RuntimeConfigSettings() {
  const { t } = useI18n();
  const [config, setConfig] = useState<AppConfig_Serialize | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [mappingText, setMappingText] = useState("[]");
  const [saved, setSaved] = useState(false);
  const [working, setWorking] = useState(true);

  useEffect(() => {
    let cancelled = false;

    void loadAppConfig()
      .then((loaded) => {
        if (cancelled) {
          return;
        }
        const normalized = withRuntimeDefaults(loaded);
        setConfig(normalized);
        setMappingText(formatMapping(normalized.CoreTypeItem));
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setError(getErrorMessage(error));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setWorking(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  function patchSection<K extends ObjectSectionKey>(key: K, patch: Partial<AppConfig_Serialize[K]>) {
    setSaved(false);
    setConfig((current) =>
      current
        ? ({
            ...current,
            [key]: {
              ...(current[key] as Record<string, unknown>),
              ...(patch as Record<string, unknown>),
            },
          } as AppConfig_Serialize)
        : current,
    );
  }

  async function save() {
    if (!config) {
      return;
    }

    setWorking(true);
    setError(null);
    setSaved(false);
    try {
      const savedConfig = await saveAppConfig({
        ...config,
        CoreTypeItem: parseMapping(mappingText),
      });
      setConfig(savedConfig);
      setMappingText(formatMapping(savedConfig.CoreTypeItem));
      setSaved(true);
    } catch (error) {
      setError(getErrorMessage(error));
    } finally {
      setWorking(false);
    }
  }

  if (!config) {
    return (
      <section className="grid gap-3">
        <h3 className="flex items-center gap-2 text-sm font-medium">
          <Settings2 className="size-4" aria-hidden="true" />
          {t("options.runtimeConfig")}
        </h3>
        <p className="text-xs text-muted-foreground">{working ? t("options.loading") : error}</p>
      </section>
    );
  }

  return (
    <section className="grid gap-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h3 className="flex items-center gap-2 text-sm font-medium">
          <Settings2 className="size-4" aria-hidden="true" />
          {t("options.runtimeConfig")}
        </h3>
        <Button disabled={working} onClick={() => void save()} type="button" variant="outline">
          <Save className="size-4" aria-hidden="true" />
          {t("actions.save")}
        </Button>
      </div>

      <Tabs defaultValue="core">
        <TabsList className="flex h-auto w-full flex-wrap justify-start">
          <TabsTrigger value="core">{t("options.runtimeCore")}</TabsTrigger>
          <TabsTrigger value="network">{t("options.runtimeNetwork")}</TabsTrigger>
          <TabsTrigger value="tests">{t("options.runtimeTests")}</TabsTrigger>
          <TabsTrigger value="mapping">{t("options.runtimeMapping")}</TabsTrigger>
        </TabsList>

        <TabsContent value="core">
          <div className="grid gap-3 md:grid-cols-2">
            <BooleanField
              checked={config.CoreBasicItem.LogEnabled}
              label="CoreBasic.LogEnabled"
              onChange={(LogEnabled) => patchSection("CoreBasicItem", { LogEnabled })}
            />
            <SelectField
              label="CoreBasic.Loglevel"
              onChange={(Loglevel) => patchSection("CoreBasicItem", { Loglevel })}
              options={["none", "trace", "debug", "info", "warn", "warning", "error"]}
              value={config.CoreBasicItem.Loglevel}
            />
            <BooleanField
              checked={config.CoreBasicItem.DefAllowInsecure}
              label="CoreBasic.DefAllowInsecure"
              onChange={(DefAllowInsecure) => patchSection("CoreBasicItem", { DefAllowInsecure })}
            />
            <BooleanField
              checked={config.CoreBasicItem.MuxEnabled}
              label="CoreBasic.MuxEnabled"
              onChange={(MuxEnabled) => patchSection("CoreBasicItem", { MuxEnabled })}
            />
            <BooleanField
              checked={config.CoreBasicItem.EnableFragment}
              label="CoreBasic.EnableFragment"
              onChange={(EnableFragment) => patchSection("CoreBasicItem", { EnableFragment })}
            />
            <BooleanField
              checked={config.CoreBasicItem.EnableCacheFile4Sbox}
              label="CoreBasic.EnableCacheFile4Sbox"
              onChange={(EnableCacheFile4Sbox) => patchSection("CoreBasicItem", { EnableCacheFile4Sbox })}
            />
            <TextField
              label="CoreBasic.DefFingerprint"
              onChange={(DefFingerprint) => patchSection("CoreBasicItem", { DefFingerprint })}
              value={config.CoreBasicItem.DefFingerprint}
            />
            <TextField
              label="CoreBasic.DefUserAgent"
              onChange={(DefUserAgent) => patchSection("CoreBasicItem", { DefUserAgent })}
              value={config.CoreBasicItem.DefUserAgent}
            />
            <TextField
              label="CoreBasic.SendThrough"
              onChange={(SendThrough) => patchSection("CoreBasicItem", { SendThrough: nullableText(SendThrough) })}
              value={config.CoreBasicItem.SendThrough ?? ""}
            />
            <TextField
              label="CoreBasic.BindInterface"
              onChange={(BindInterface) => patchSection("CoreBasicItem", { BindInterface: nullableText(BindInterface) })}
              value={config.CoreBasicItem.BindInterface ?? ""}
            />
            <NumberField
              label="Mux4Ray.Concurrency"
              onChange={(Concurrency) => patchSection("Mux4RayItem", { Concurrency })}
              value={config.Mux4RayItem.Concurrency ?? null}
            />
            <NumberField
              label="Mux4Ray.XudpConcurrency"
              onChange={(XudpConcurrency) => patchSection("Mux4RayItem", { XudpConcurrency })}
              value={config.Mux4RayItem.XudpConcurrency ?? null}
            />
            <TextField
              label="Mux4Ray.XudpProxyUDP443"
              onChange={(XudpProxyUDP443) => patchSection("Mux4RayItem", { XudpProxyUDP443: nullableText(XudpProxyUDP443) })}
              value={config.Mux4RayItem.XudpProxyUDP443 ?? ""}
            />
            <TextField
              label="Mux4Sbox.Protocol"
              onChange={(Protocol) => patchSection("Mux4SboxItem", { Protocol })}
              value={config.Mux4SboxItem.Protocol}
            />
            <NumberField
              label="Mux4Sbox.MaxConnections"
              onChange={(MaxConnections) => patchSection("Mux4SboxItem", { MaxConnections: MaxConnections ?? 0 })}
              value={config.Mux4SboxItem.MaxConnections}
            />
            <BooleanField
              checked={config.Mux4SboxItem.Padding ?? false}
              label="Mux4Sbox.Padding"
              onChange={(Padding) => patchSection("Mux4SboxItem", { Padding })}
            />
            <TextField
              label="Fragment4Ray.Packets"
              onChange={(Packets) => patchSection("Fragment4RayItem", { Packets: nullableText(Packets) })}
              value={config.Fragment4RayItem.Packets ?? ""}
            />
            <TextField
              label="Fragment4Ray.Length"
              onChange={(Length) => patchSection("Fragment4RayItem", { Length: nullableText(Length) })}
              value={config.Fragment4RayItem.Length ?? ""}
            />
            <TextField
              label="Fragment4Ray.Interval"
              onChange={(Interval) => patchSection("Fragment4RayItem", { Interval: nullableText(Interval) })}
              value={config.Fragment4RayItem.Interval ?? ""}
            />
          </div>
        </TabsContent>

        <TabsContent value="network">
          <div className="grid gap-3 md:grid-cols-2">
            <BooleanField
              checked={config.TunModeItem.EnableTun ?? false}
              label="Tun.EnableTun"
              onChange={(EnableTun) => patchSection("TunModeItem", { EnableTun })}
            />
            <BooleanField
              checked={config.TunModeItem.AutoRoute ?? false}
              label="Tun.AutoRoute"
              onChange={(AutoRoute) => patchSection("TunModeItem", { AutoRoute })}
            />
            <BooleanField
              checked={config.TunModeItem.StrictRoute ?? false}
              label="Tun.StrictRoute"
              onChange={(StrictRoute) => patchSection("TunModeItem", { StrictRoute })}
            />
            <BooleanField
              checked={config.TunModeItem.EnableIPv6Address ?? false}
              label="Tun.EnableIPv6Address"
              onChange={(EnableIPv6Address) => patchSection("TunModeItem", { EnableIPv6Address })}
            />
            <BooleanField
              checked={config.TunModeItem.EnableLegacyProtect ?? false}
              label="Tun.EnableLegacyProtect"
              onChange={(EnableLegacyProtect) => patchSection("TunModeItem", { EnableLegacyProtect })}
            />
            <TextField
              label="Tun.Stack"
              onChange={(Stack) => patchSection("TunModeItem", { Stack })}
              value={config.TunModeItem.Stack ?? ""}
            />
            <NumberField
              label="Tun.Mtu"
              onChange={(Mtu) => patchSection("TunModeItem", { Mtu: Mtu ?? 0 })}
              value={config.TunModeItem.Mtu ?? null}
            />
            <TextField
              label="Tun.IcmpRouting"
              onChange={(IcmpRouting) => patchSection("TunModeItem", { IcmpRouting })}
              value={config.TunModeItem.IcmpRouting ?? ""}
            />
            <SelectField
              label="SystemProxy.SysProxyType"
              onChange={(value) => patchSection("SystemProxyItem", { SysProxyType: Number(value) })}
              options={sysProxyOptions.map((option) => String(option.value))}
              optionLabel={(value) => sysProxyOptions.find((option) => option.value === Number(value))?.label ?? value}
              value={String(config.SystemProxyItem.SysProxyType)}
            />
            <BooleanField
              checked={config.SystemProxyItem.NotProxyLocalAddress}
              label="SystemProxy.NotProxyLocalAddress"
              onChange={(NotProxyLocalAddress) => patchSection("SystemProxyItem", { NotProxyLocalAddress })}
            />
            <TextField
              label="SystemProxy.Exceptions"
              onChange={(SystemProxyExceptions) => patchSection("SystemProxyItem", { SystemProxyExceptions })}
              value={config.SystemProxyItem.SystemProxyExceptions}
            />
            <TextField
              label="SystemProxy.AdvancedProtocol"
              onChange={(SystemProxyAdvancedProtocol) => patchSection("SystemProxyItem", { SystemProxyAdvancedProtocol })}
              value={config.SystemProxyItem.SystemProxyAdvancedProtocol}
            />
            <TextField
              label="SystemProxy.CustomPacPath"
              onChange={(CustomSystemProxyPacPath) =>
                patchSection("SystemProxyItem", { CustomSystemProxyPacPath: nullableText(CustomSystemProxyPacPath) })
              }
              value={config.SystemProxyItem.CustomSystemProxyPacPath ?? ""}
            />
            <TextField
              label="SystemProxy.CustomScriptPath"
              onChange={(CustomSystemProxyScriptPath) =>
                patchSection("SystemProxyItem", { CustomSystemProxyScriptPath: nullableText(CustomSystemProxyScriptPath) })
              }
              value={config.SystemProxyItem.CustomSystemProxyScriptPath ?? ""}
            />
          </div>
        </TabsContent>

        <TabsContent value="tests">
          <div className="grid gap-3 md:grid-cols-2">
            <NumberField
              label="SpeedTest.Timeout"
              onChange={(SpeedTestTimeout) => patchSection("SpeedTestItem", { SpeedTestTimeout: SpeedTestTimeout ?? 0 })}
              value={config.SpeedTestItem.SpeedTestTimeout}
            />
            <NumberField
              label="SpeedTest.MixedConcurrency"
              onChange={(MixedConcurrencyCount) => patchSection("SpeedTestItem", { MixedConcurrencyCount: MixedConcurrencyCount ?? 0 })}
              value={config.SpeedTestItem.MixedConcurrencyCount}
            />
            <TextField
              label="SpeedTest.Url"
              onChange={(SpeedTestUrl) => patchSection("SpeedTestItem", { SpeedTestUrl })}
              value={config.SpeedTestItem.SpeedTestUrl}
            />
            <TextField
              label="SpeedTest.PingUrl"
              onChange={(SpeedPingTestUrl) => patchSection("SpeedTestItem", { SpeedPingTestUrl })}
              value={config.SpeedTestItem.SpeedPingTestUrl}
            />
            <TextField
              label="SpeedTest.IPAPIUrl"
              onChange={(IPAPIUrl) => patchSection("SpeedTestItem", { IPAPIUrl })}
              value={config.SpeedTestItem.IPAPIUrl}
            />
            <TextField
              label="SpeedTest.UdpTarget"
              onChange={(UdpTestTarget) => patchSection("SpeedTestItem", { UdpTestTarget })}
              value={config.SpeedTestItem.UdpTestTarget}
            />
            <NumberField
              label="SpeedTest.PageSize"
              onChange={(SpeedTestPageSize) => patchSection("SpeedTestItem", { SpeedTestPageSize })}
              value={config.SpeedTestItem.SpeedTestPageSize ?? null}
            />
            <NumberField
              label="SpeedTest.DelayInterval"
              onChange={(SpeedTestDelayInterval) => patchSection("SpeedTestItem", { SpeedTestDelayInterval })}
              value={config.SpeedTestItem.SpeedTestDelayInterval ?? null}
            />
            <NumberField
              label="Hysteria.UpMbps"
              onChange={(UpMbps) => patchSection("HysteriaItem", { UpMbps: UpMbps ?? undefined })}
              value={config.HysteriaItem.UpMbps ?? null}
            />
            <NumberField
              label="Hysteria.DownMbps"
              onChange={(DownMbps) => patchSection("HysteriaItem", { DownMbps: DownMbps ?? undefined })}
              value={config.HysteriaItem.DownMbps ?? null}
            />
            <NumberField
              label="Hysteria.HopInterval"
              onChange={(HopInterval) => patchSection("HysteriaItem", { HopInterval: HopInterval ?? undefined })}
              value={config.HysteriaItem.HopInterval ?? null}
            />
            <BooleanField
              checked={config.CheckUpdateItem.CheckPreReleaseUpdate}
              label="Update.CheckPreRelease"
              onChange={(CheckPreReleaseUpdate) => patchSection("CheckUpdateItem", { CheckPreReleaseUpdate })}
            />
            <TextField
              label="Update.SelectedTargets"
              onChange={(value) =>
                patchSection("CheckUpdateItem", {
                  SelectedCoreTypes: value.split(",").map((item) => item.trim()).filter(Boolean),
                })
              }
              value={config.CheckUpdateItem.SelectedCoreTypes?.join(", ") ?? ""}
            />
            <TextField
              label="Const.CdnBaseUrl"
              onChange={(CdnBaseUrl) => patchSection("ConstItem", { CdnBaseUrl: nullableText(CdnBaseUrl) })}
              value={config.ConstItem.CdnBaseUrl ?? ""}
            />
            <TextField
              label="Const.CdnReleaseIndexUrl"
              onChange={(CdnReleaseIndexUrl) => patchSection("ConstItem", { CdnReleaseIndexUrl: nullableText(CdnReleaseIndexUrl) })}
              value={config.ConstItem.CdnReleaseIndexUrl ?? ""}
            />
            <TextField
              label="Const.CdnCoreManifestUrl"
              onChange={(CdnCoreManifestUrl) => patchSection("ConstItem", { CdnCoreManifestUrl: nullableText(CdnCoreManifestUrl) })}
              value={config.ConstItem.CdnCoreManifestUrl ?? ""}
            />
            <TextField
              label="Const.SubConvertUrl"
              onChange={(SubConvertUrl) => patchSection("ConstItem", { SubConvertUrl: nullableText(SubConvertUrl) })}
              value={config.ConstItem.SubConvertUrl ?? ""}
            />
          </div>
        </TabsContent>

        <TabsContent value="mapping">
          <div className="grid gap-3">
            <div className="grid gap-1">
              <Label className="flex items-center gap-2 text-xs text-muted-foreground" htmlFor="core-type-mapping">
                <Braces className="size-4" aria-hidden="true" />
                CoreTypeItem
              </Label>
              <Textarea
                className="min-h-44 resize-y bg-card font-mono text-xs"
                id="core-type-mapping"
                onChange={(event) => {
                  setSaved(false);
                  setMappingText(event.currentTarget.value);
                }}
                value={mappingText}
              />
            </div>
            <div className="grid gap-3 md:grid-cols-2">
              <TextField
                label="Const.GeoSourceUrl"
                onChange={(GeoSourceUrl) => patchSection("ConstItem", { GeoSourceUrl: nullableText(GeoSourceUrl) })}
                value={config.ConstItem.GeoSourceUrl ?? ""}
              />
              <TextField
                label="Const.SrsSourceUrl"
                onChange={(SrsSourceUrl) => patchSection("ConstItem", { SrsSourceUrl: nullableText(SrsSourceUrl) })}
                value={config.ConstItem.SrsSourceUrl ?? ""}
              />
              <TextField
                label="Const.RouteRulesTemplateSourceUrl"
                onChange={(RouteRulesTemplateSourceUrl) =>
                  patchSection("ConstItem", { RouteRulesTemplateSourceUrl: nullableText(RouteRulesTemplateSourceUrl) })
                }
                value={config.ConstItem.RouteRulesTemplateSourceUrl ?? ""}
              />
            </div>
          </div>
        </TabsContent>
      </Tabs>

      <div className="flex flex-wrap items-center gap-2">
        {saved ? <span className="text-xs text-muted-foreground">{t("options.saved")}</span> : null}
        {error ? <span className="text-xs text-destructive">{error}</span> : null}
      </div>
    </section>
  );
}

function BooleanField({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <Label className="flex h-9 cursor-pointer items-center justify-between gap-3 rounded-md border bg-card px-3 text-sm">
      <span className="truncate">{label}</span>
      <Switch checked={checked} onCheckedChange={onChange} />
    </Label>
  );
}

function TextField({
  label,
  onChange,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  return (
    <div className="grid min-w-0 gap-1">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <Input className="bg-card" onChange={(event) => onChange(event.currentTarget.value)} value={value} />
    </div>
  );
}

function NumberField({
  label,
  onChange,
  value,
}: {
  label: string;
  onChange: (value: number | null) => void;
  value: number | null;
}) {
  return (
    <div className="grid min-w-0 gap-1">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <Input
        className="bg-card"
        onChange={(event) => {
          const text = event.currentTarget.value.trim();
          onChange(text ? Number(text) : null);
        }}
        type="number"
        value={value ?? ""}
      />
    </div>
  );
}

function SelectField({
  label,
  onChange,
  optionLabel = (value) => value,
  options,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  optionLabel?: (value: string) => string;
  options: string[];
  value: string;
}) {
  return (
    <div className="grid min-w-0 gap-1">
      <Label className="text-xs text-muted-foreground">{label}</Label>
      <Select onValueChange={onChange} value={value}>
        <SelectTrigger className="bg-card">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option} value={option}>
              {optionLabel(option)}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

function nullableText(value: string): string | null {
  return value.trim() ? value : null;
}

function formatMapping(value: CoreTypeItem[]): string {
  return JSON.stringify(value ?? [], null, 2);
}

function parseMapping(value: string): CoreTypeItem[] {
  const parsed = JSON.parse(value) as unknown;
  if (!Array.isArray(parsed)) {
    throw new Error("CoreTypeItem must be a JSON array.");
  }

  return parsed.map((item) => {
    if (!item || typeof item !== "object" || Array.isArray(item)) {
      throw new Error("CoreTypeItem entries must be JSON objects.");
    }

    const record = item as Record<string, unknown>;
    return {
      ConfigType: optionalInteger(record.ConfigType),
      CoreType: optionalInteger(record.CoreType),
    };
  });
}

function optionalInteger(value: unknown): number | undefined {
  if (value === null || value === undefined || value === "") {
    return undefined;
  }

  const numberValue = Number(value);
  if (!Number.isInteger(numberValue)) {
    throw new Error("CoreTypeItem ConfigType/CoreType must be integers.");
  }

  return numberValue;
}

function withRuntimeDefaults(config: AppConfig_Serialize): AppConfig_Serialize {
  const loose = config as AppConfig_Serialize & Record<string, unknown>;

  return {
    ...config,
    CheckUpdateItem: {
      CheckPreReleaseUpdate: false,
      ...(loose.CheckUpdateItem as Record<string, unknown> | undefined),
    },
    ConstItem: {
      ...(loose.ConstItem as Record<string, unknown> | undefined),
    },
    CoreBasicItem: {
      BindInterface: null,
      DefAllowInsecure: false,
      DefFingerprint: "",
      DefUserAgent: "",
      EnableCacheFile4Sbox: false,
      EnableFragment: false,
      LogEnabled: false,
      Loglevel: "warning",
      MuxEnabled: false,
      SendThrough: null,
      ...(loose.CoreBasicItem as Record<string, unknown> | undefined),
    },
    CoreTypeItem: Array.isArray(loose.CoreTypeItem) ? (loose.CoreTypeItem as CoreTypeItem[]) : [],
    Fragment4RayItem: {
      ...(loose.Fragment4RayItem as Record<string, unknown> | undefined),
    },
    HysteriaItem: {
      ...(loose.HysteriaItem as Record<string, unknown> | undefined),
    },
    Mux4RayItem: {
      ...(loose.Mux4RayItem as Record<string, unknown> | undefined),
    },
    Mux4SboxItem: {
      MaxConnections: 0,
      Protocol: "",
      ...(loose.Mux4SboxItem as Record<string, unknown> | undefined),
    },
    SpeedTestItem: {
      IPAPIUrl: "",
      MixedConcurrencyCount: 0,
      SpeedPingTestUrl: "",
      SpeedTestTimeout: 0,
      SpeedTestUrl: "",
      UdpTestTarget: "",
      ...(loose.SpeedTestItem as Record<string, unknown> | undefined),
    },
    SystemProxyItem: {
      NotProxyLocalAddress: false,
      SysProxyType: 0,
      SystemProxyAdvancedProtocol: "",
      SystemProxyExceptions: "",
      ...(loose.SystemProxyItem as Record<string, unknown> | undefined),
    },
    TunModeItem: {
      ...(loose.TunModeItem as Record<string, unknown> | undefined),
    },
  } as AppConfig_Serialize;
}
