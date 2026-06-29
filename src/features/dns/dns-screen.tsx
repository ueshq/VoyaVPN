import { useId, useMemo, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { json } from "@codemirror/lang-json";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Braces, Database, RefreshCw, RotateCcw, Save, ServerCog, ShieldCheck, TriangleAlert } from "lucide-react";
import { z } from "zod";

import { PageHeader, PageHeaderHeading, PageSection } from "@/components/app-shell/page-section";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { IpcCommandError, loadDnsSettings, saveDnsSettings } from "@/ipc";
import type { DnsItem_Serialize, DnsSettings_Deserialize, DnsSettings_Serialize } from "@/ipc/bindings";
import { cn, getErrorMessage } from "@/lib/utils";

const STRATEGIES = ["", "AsIs", "UseIP", "UseIPv4", "UseIPv6", "ForceIPv4", "ForceIPv6"] as const;
const EMPTY_SELECT_VALUE = "__voyavpn_empty_select_value__";
const editorExtensions = [json()];
const optionalNullableText = z.string().nullable().optional();

const simpleDnsItemSchema = z.object({
  UseSystemHosts: z.boolean().nullable().optional(),
  AddCommonHosts: z.boolean().nullable().optional(),
  FakeIP: z.boolean().nullable().optional(),
  GlobalFakeIp: z.boolean().nullable().optional(),
  BlockBindingQuery: z.boolean().nullable().optional(),
  DirectDNS: optionalNullableText,
  RemoteDNS: optionalNullableText,
  BootstrapDNS: optionalNullableText,
  Strategy4Freedom: z.enum(STRATEGIES).nullable().optional(),
  Strategy4Proxy: z.enum(STRATEGIES).nullable().optional(),
  ServeStale: z.boolean().nullable().optional(),
  ParallelQuery: z.boolean().nullable().optional(),
  Hosts: optionalNullableText.superRefine(validateHosts),
  DirectExpectedIPs: optionalNullableText.superRefine(validateExpectedIps),
});

const dnsItemSchema = z.object({
  Id: z.string().optional(),
  Remarks: z.string().optional(),
  Enabled: z.boolean().optional(),
  CoreType: z.number().int().optional(),
  UseSystemHosts: z.boolean().optional(),
  NormalDNS: optionalNullableText,
  TunDNS: optionalNullableText,
  DomainStrategy4Freedom: optionalNullableText,
  DomainDNSAddress: optionalNullableText,
});

const dnsSettingsSchema: z.ZodType<DnsSettings_Deserialize> = z.object({
  simpleDnsItem: simpleDnsItemSchema,
  singboxDnsItem: dnsItemSchema.extend({
    NormalDNS: optionalNullableText.superRefine((value, context) => validateSingboxDnsJson(value, context)),
    TunDNS: optionalNullableText.superRefine((value, context) => validateSingboxDnsJson(value, context)),
  }),
  defaults: z.object({
    singboxNormalDns: z.string(),
    singboxTunDns: z.string(),
  }),
});

type ErrorMap = Record<string, string>;
type DnsCoreKey = "singboxDnsItem";

export function DnsScreen() {
  const queryClient = useQueryClient();
  const dnsQuery = useQuery({
    queryFn: loadDnsSettings,
    queryKey: ["dns"],
  });
  const [draft, setDraft] = useState<DnsSettings_Serialize | null>(null);
  const [operationError, setOperationError] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<ErrorMap>({});
  const form = draft ?? dnsQuery.data ?? null;

  const issueCount = Object.keys(fieldErrors).length;
  const isDirty = useMemo(() => {
    if (!form || !dnsQuery.data) {
      return false;
    }
    return draft !== null && JSON.stringify(draft) !== JSON.stringify(dnsQuery.data);
  }, [dnsQuery.data, draft, form]);

  async function handleReload() {
    setOperationError(null);
    setFieldErrors({});
    setDraft(null);
    await queryClient.invalidateQueries({ queryKey: ["dns"] });
  }

  async function handleSave() {
    if (!form) {
      return;
    }
    setOperationError(null);
    setFieldErrors({});
    try {
      const payload = dnsSettingsSchema.parse(form);
      const saved = await saveDnsSettings(payload);
      queryClient.setQueryData(["dns"], saved);
      setDraft(null);
      await queryClient.invalidateQueries({ queryKey: ["dns"] });
      await queryClient.invalidateQueries({ queryKey: ["app-config"] });
    } catch (error) {
      if (error instanceof z.ZodError) {
        setOperationError("DNS settings validation failed");
        setFieldErrors(zodIssuesToErrorMap(error));
        return;
      }
      if (error instanceof IpcCommandError && error.appError.kind === "dns") {
        setOperationError(error.appError.message.message);
        setFieldErrors(
          Object.fromEntries(error.appError.message.issues.map((issue) => [issue.field, issue.message])),
        );
        return;
      }
      setOperationError(getErrorMessage(error));
    }
  }

  function updateSimple(patch: Partial<DnsSettings_Serialize["simpleDnsItem"]>) {
    setDraft((current) =>
      current
        ? {
            ...current,
            simpleDnsItem: {
              ...current.simpleDnsItem,
              ...patch,
            },
          }
        : dnsQuery.data
          ? {
              ...dnsQuery.data,
              simpleDnsItem: {
                ...dnsQuery.data.simpleDnsItem,
                ...patch,
              },
            }
          : current,
    );
  }

  function updateCore(core: DnsCoreKey, patch: Partial<DnsItem_Serialize>) {
    setDraft((current) =>
      current
        ? {
            ...current,
            [core]: {
              ...current[core],
              ...patch,
            },
          }
        : dnsQuery.data
          ? {
              ...dnsQuery.data,
              [core]: {
                ...dnsQuery.data[core],
                ...patch,
              },
            }
          : current,
    );
  }

  return (
    <PageSection aria-label="DNS">
      <PageHeader>
        <PageHeaderHeading icon={Database} title="DNS">
          <Badge variant="outline">
            {form?.simpleDnsItem.FakeIP ? "FakeIP" : "Standard"}
          </Badge>
          {issueCount ? (
            <Badge variant="destructive">
              <TriangleAlert className="size-3.5" aria-hidden="true" />
              {issueCount} errors
            </Badge>
          ) : null}
        </PageHeaderHeading>

        <div className="ms-auto flex items-center gap-2">
          <Button disabled={dnsQuery.isFetching} onClick={() => void handleReload()} size="sm" type="button" variant="outline">
            <RefreshCw className={cn("size-4", dnsQuery.isFetching ? "animate-spin" : "")} aria-hidden="true" />
            Reload
          </Button>
          <Button disabled={!form || !isDirty} onClick={() => void handleSave()} size="sm" type="button">
            <Save className="size-4" aria-hidden="true" />
            Save
          </Button>
        </div>
      </PageHeader>

      {operationError ? (
        <div className="border-b px-4 py-2">
          <Alert className="py-2" variant="destructive">
            <TriangleAlert aria-hidden="true" />
            <AlertDescription>{operationError}</AlertDescription>
          </Alert>
        </div>
      ) : null}

      <div className="grid min-h-0 flex-1 grid-cols-1 lg:grid-cols-[22rem_1fr]">
        <aside className="min-h-0 border-b bg-surface-sunken lg:border-b-0 lg:border-e">
          <ScrollArea className="h-[32rem] lg:h-full">
            <div className="p-4">
              {form ? (
                <SimpleDnsForm errors={fieldErrors} settings={form} updateSimple={updateSimple} />
              ) : (
                <div className="text-sm text-muted-foreground">Loading DNS settings</div>
              )}
            </div>
          </ScrollArea>
        </aside>

        <div className="min-h-0 overflow-hidden">
          {form ? (
            <Tabs className="flex h-full min-h-0 flex-col" defaultValue="singbox">
              <div className="shrink-0 border-b px-4 py-2">
                <TabsList>
                  <TabsTrigger value="singbox">
                    <ServerCog className="size-4" aria-hidden="true" />
                    sing-box
                  </TabsTrigger>
                </TabsList>
              </div>
              <TabsContent className="m-0 min-h-0 flex-1" value="singbox">
                <AdvancedDnsEditor
                  defaults={{
                    normal: form.defaults.singboxNormalDns,
                    tun: form.defaults.singboxTunDns,
                  }}
                  errors={fieldErrors}
                  fieldPrefix="singboxDnsItem"
                  item={form.singboxDnsItem}
                  onChange={(patch) => updateCore("singboxDnsItem", patch)}
                  title="sing-box raw DNS"
                />
              </TabsContent>
            </Tabs>
          ) : null}
        </div>
      </div>
    </PageSection>
  );
}

function validateHosts(value: string | null | undefined, context: z.RefinementCtx) {
  if (!value) {
    return;
  }

  value.split(/\r?\n/).forEach((line, index) => {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) {
      return;
    }
    if (trimmed.split(/\s+/).length < 2) {
      context.addIssue({
        code: "custom",
        message: `Host line ${index + 1} must contain a domain and at least one answer`,
      });
    }
  });
}

function validateExpectedIps(value: string | null | undefined, context: z.RefinementCtx) {
  if (!value) {
    return;
  }

  if (
    value
      .split(",")
      .map((part) => part.trim())
      .some((part) => part !== "" && /\s/.test(part))
  ) {
    context.addIssue({
      code: "custom",
      message: "Expected IPs must be comma-separated without embedded whitespace",
    });
  }
}

function validateSingboxDnsJson(value: string | null | undefined, context: z.RefinementCtx) {
  const parsed = parseJsonObject(value, "Invalid sing-box DNS JSON", context);
  if (!parsed) {
    return;
  }

  const servers = parsed.servers;
  if (!Array.isArray(servers) || servers.length === 0) {
    context.addIssue({
      code: "custom",
      message: "sing-box DNS JSON must contain at least one server",
    });
    return;
  }

  if (
    servers.some(
      (server) =>
        !server ||
        typeof server !== "object" ||
        typeof (server as Record<string, unknown>).type !== "string" ||
        (server as Record<string, unknown>).type === "",
    )
  ) {
    context.addIssue({
      code: "custom",
      message: "Every sing-box DNS server must include a non-empty type",
    });
  }
}

function parseJsonObject(
  value: string | null | undefined,
  label: string,
  context: z.RefinementCtx,
) {
  const trimmed = value?.trim();
  if (!trimmed) {
    return null;
  }

  try {
    const parsed: unknown = JSON.parse(trimmed);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      context.addIssue({ code: "custom", message: `${label}: expected a JSON object` });
      return null;
    }
    return parsed as Record<string, unknown>;
  } catch (error) {
    context.addIssue({
      code: "custom",
      message: `${label}: ${getErrorMessage(error)}`,
    });
    return null;
  }
}

function zodIssuesToErrorMap(error: z.ZodError): ErrorMap {
  return Object.fromEntries(
    error.issues.map((issue) => [issue.path.join(".") || "form", issue.message]),
  );
}

function SimpleDnsForm({
  errors,
  settings,
  updateSimple,
}: {
  errors: ErrorMap;
  settings: DnsSettings_Serialize;
  updateSimple: (patch: Partial<DnsSettings_Serialize["simpleDnsItem"]>) => void;
}) {
  const simple = settings.simpleDnsItem;

  return (
    <Card className="gap-3 rounded-xl bg-surface-raised p-3 shadow-raised">
      <CardHeader className="p-0">
        <CardTitle className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground">
          <ShieldCheck className="size-4 text-muted-foreground" aria-hidden="true" />
          Simple DNS
        </CardTitle>
      </CardHeader>
      <CardContent className="grid gap-4 p-0">
        <div className="grid gap-2">
          <CheckboxField
            checked={Boolean(simple.UseSystemHosts)}
            label="System hosts"
            onChange={(value) => updateSimple({ UseSystemHosts: value })}
          />
          <CheckboxField
            checked={Boolean(simple.AddCommonHosts)}
            label="Common hosts"
            onChange={(value) => updateSimple({ AddCommonHosts: value })}
          />
          <CheckboxField
            checked={Boolean(simple.BlockBindingQuery)}
            label="Block HTTPS/SVCB"
            onChange={(value) => updateSimple({ BlockBindingQuery: value })}
          />
          <CheckboxField
            checked={Boolean(simple.ServeStale)}
            label="Serve stale"
            onChange={(value) => updateSimple({ ServeStale: value })}
          />
          <CheckboxField
            checked={Boolean(simple.ParallelQuery)}
            label="Parallel query"
            onChange={(value) => updateSimple({ ParallelQuery: value })}
          />
          <CheckboxField
            checked={Boolean(simple.FakeIP)}
            label="FakeIP"
            onChange={(value) => updateSimple({ FakeIP: value })}
          />
          <CheckboxField
            checked={Boolean(simple.GlobalFakeIp)}
            disabled={!simple.FakeIP}
            label="Global FakeIP"
            onChange={(value) => updateSimple({ GlobalFakeIp: value })}
          />
        </div>

        <TextField
          label="Direct DNS"
          onChange={(value) => updateSimple({ DirectDNS: value })}
          value={simple.DirectDNS ?? ""}
        />
        <TextField
          label="Remote DNS"
          onChange={(value) => updateSimple({ RemoteDNS: value })}
          value={simple.RemoteDNS ?? ""}
        />
        <TextField
          label="Bootstrap DNS"
          onChange={(value) => updateSimple({ BootstrapDNS: value })}
          value={simple.BootstrapDNS ?? ""}
        />

        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-1">
          <SelectField
            label="Direct strategy"
            onChange={(value) => updateSimple({ Strategy4Freedom: value || null })}
            value={simple.Strategy4Freedom ?? ""}
          />
          <SelectField
            label="Proxy strategy"
            onChange={(value) => updateSimple({ Strategy4Proxy: value || null })}
            value={simple.Strategy4Proxy ?? ""}
          />
        </div>

        <TextAreaField
          error={errors["simpleDnsItem.hosts"]}
          label="Hosts"
          onChange={(value) => updateSimple({ Hosts: value })}
          value={simple.Hosts ?? ""}
        />
        <TextAreaField
          error={errors["simpleDnsItem.directExpectedIPs"]}
          label="Expected IPs"
          onChange={(value) => updateSimple({ DirectExpectedIPs: value })}
          value={simple.DirectExpectedIPs ?? ""}
        />
      </CardContent>
    </Card>
  );
}

function AdvancedDnsEditor({
  defaults,
  errors,
  fieldPrefix,
  item,
  onChange,
  showSystemHosts = false,
  title,
}: {
  defaults: { normal: string; tun: string };
  errors: ErrorMap;
  fieldPrefix: string;
  item: DnsItem_Serialize;
  onChange: (patch: Partial<DnsItem_Serialize>) => void;
  showSystemHosts?: boolean;
  title: string;
}) {
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex min-h-12 shrink-0 flex-wrap items-center gap-2 border-b px-4 py-2">
        <h3 className="text-sm font-semibold">{title}</h3>
        <CheckboxField
          checked={item.Enabled}
          className="ms-auto"
          label="Enabled"
          onChange={(value) => onChange({ Enabled: value })}
        />
      </div>

      <ScrollArea className="min-h-0 flex-1 bg-surface-sunken">
        <div className="grid gap-4 p-4 xl:grid-cols-2">
          <JsonEditorField
            error={errors[`${fieldPrefix}.normalDNS`]}
            label="Normal DNS"
            onChange={(value) => onChange({ NormalDNS: value })}
            onReset={() => onChange({ NormalDNS: defaults.normal })}
            value={item.NormalDNS ?? ""}
          />
          <JsonEditorField
            error={errors[`${fieldPrefix}.tunDNS`]}
            label="TUN DNS"
            onChange={(value) => onChange({ TunDNS: value })}
            onReset={() => onChange({ TunDNS: defaults.tun })}
            value={item.TunDNS ?? ""}
          />
          <TextField
            label="Direct strategy"
            onChange={(value) => onChange({ DomainStrategy4Freedom: value || null })}
            value={item.DomainStrategy4Freedom ?? ""}
          />
          <TextField
            label="Domain DNS address"
            onChange={(value) => onChange({ DomainDNSAddress: value || null })}
            value={item.DomainDNSAddress ?? ""}
          />
          {showSystemHosts ? (
            <CheckboxField
              checked={item.UseSystemHosts}
              label="System hosts"
              onChange={(value) => onChange({ UseSystemHosts: value })}
            />
          ) : null}
        </div>
      </ScrollArea>
    </div>
  );
}

function JsonEditorField({
  error,
  label,
  onChange,
  onReset,
  value,
}: {
  error?: string;
  label: string;
  onChange: (value: string) => void;
  onReset: () => void;
  value: string;
}) {
  return (
    <Card className="min-h-[22rem] gap-3 rounded-xl bg-surface-raised p-3 shadow-raised">
      <CardHeader className="p-0">
        <CardTitle className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground">
          <Braces className="size-4 text-muted-foreground" aria-hidden="true" />
          {label}
          <Button className="ms-auto h-7 px-2 normal-case" onClick={onReset} type="button" variant="outline">
            <RotateCcw className="size-3.5" aria-hidden="true" />
            Default
          </Button>
        </CardTitle>
      </CardHeader>
      <CardContent className="grid gap-2 p-0">
        <div
          aria-invalid={error ? true : undefined}
          className={cn(
            "overflow-hidden rounded-md border border-input bg-background shadow-xs transition-[color,box-shadow] focus-within:border-accent-blue focus-within:ring-[3px] focus-within:ring-accent-blue/40 dark:bg-input/30",
            error ? "border-destructive ring-destructive/20 dark:ring-destructive/40" : "",
          )}
        >
          <CodeMirror
            basicSetup={{
              foldGutter: true,
              highlightActiveLine: true,
              lineNumbers: true,
            }}
            extensions={editorExtensions}
            height="20rem"
            onChange={onChange}
            value={value}
          />
        </div>
        {error ? <span className="text-xs text-destructive">{error}</span> : null}
      </CardContent>
    </Card>
  );
}

function CheckboxField({
  checked,
  className,
  disabled = false,
  label,
  onChange,
}: {
  checked: boolean;
  className?: string;
  disabled?: boolean;
  label: string;
  onChange: (value: boolean) => void;
}) {
  const id = useId();

  return (
    <div className={cn("flex items-center gap-2", disabled ? "opacity-55" : "", className)}>
      <Checkbox
        checked={checked}
        disabled={disabled}
        id={id}
        onCheckedChange={(nextChecked) => onChange(nextChecked === true)}
      />
      <Label className={cn("text-sm", disabled ? "cursor-not-allowed" : "cursor-pointer")} htmlFor={id}>
        {label}
      </Label>
    </div>
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
  const id = useId();

  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
    </div>
  );
}

function SelectField({
  label,
  onChange,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  const id = useId();
  const selectValue = value === "" ? EMPTY_SELECT_VALUE : value;

  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Select
        onValueChange={(nextValue) => onChange(nextValue === EMPTY_SELECT_VALUE ? "" : nextValue)}
        value={selectValue}
      >
        <SelectTrigger className="w-full" id={id}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {STRATEGIES.map((strategy) => (
            <SelectItem key={strategy || EMPTY_SELECT_VALUE} value={strategy || EMPTY_SELECT_VALUE}>
              {strategy || "default"}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

function TextAreaField({
  error,
  label,
  onChange,
  value,
}: {
  error?: string;
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  const id = useId();
  const errorId = `${id}-error`;

  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Textarea
        aria-describedby={error ? errorId : undefined}
        aria-invalid={error ? true : undefined}
        className="min-h-24 resize-y"
        id={id}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
      {error ? (
        <span className="text-xs text-destructive" id={errorId}>
          {error}
        </span>
      ) : null}
    </div>
  );
}
