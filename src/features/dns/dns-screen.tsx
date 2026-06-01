import { useMemo, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { json } from "@codemirror/lang-json";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Braces, Database, RefreshCw, RotateCcw, Save, ServerCog, ShieldCheck, TriangleAlert } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { IpcCommandError, loadDnsSettings, saveDnsSettings } from "@/ipc";
import type { DnsItem_Serialize, DnsSettings_Deserialize, DnsSettings_Serialize } from "@/ipc/bindings";
import { cn } from "@/lib/utils";

const STRATEGIES = ["", "AsIs", "UseIP", "UseIPv4", "UseIPv6", "ForceIPv4", "ForceIPv6"];
const editorExtensions = [json()];

type ErrorMap = Record<string, string>;
type DnsCoreKey = "xrayDnsItem" | "singboxDnsItem";

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
      const saved = await saveDnsSettings(form as DnsSettings_Deserialize);
      queryClient.setQueryData(["dns"], saved);
      setDraft(null);
      await queryClient.invalidateQueries({ queryKey: ["dns"] });
      await queryClient.invalidateQueries({ queryKey: ["app-config"] });
    } catch (error) {
      if (error instanceof IpcCommandError && error.appError.kind === "dns") {
        setOperationError(error.appError.message.message);
        setFieldErrors(
          Object.fromEntries(error.appError.message.issues.map((issue) => [issue.field, issue.message])),
        );
        return;
      }
      setOperationError(error instanceof Error ? error.message : String(error));
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
    <section className="flex h-full min-h-0 flex-col" aria-label="DNS">
      <div className="flex min-h-14 shrink-0 flex-wrap items-center gap-2 border-b px-4 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <Database className="size-4 text-muted-foreground" aria-hidden="true" />
          <h2 className="text-sm font-semibold">DNS</h2>
          <span className="rounded-md border px-2 py-1 text-xs text-muted-foreground">
            {form?.simpleDnsItem.FakeIP ? "FakeIP" : "Standard"}
          </span>
          {issueCount ? (
            <span className="inline-flex items-center gap-1 rounded-md border border-destructive/40 px-2 py-1 text-xs text-destructive">
              <TriangleAlert className="size-3.5" aria-hidden="true" />
              {issueCount} errors
            </span>
          ) : null}
        </div>

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
      </div>

      {operationError ? (
        <div className="border-b bg-destructive/10 px-4 py-2 text-sm text-destructive">{operationError}</div>
      ) : null}

      <div className="grid min-h-0 flex-1 grid-cols-1 lg:grid-cols-[22rem_1fr]">
        <aside className="min-h-0 overflow-auto border-b p-4 lg:border-b-0 lg:border-e">
          {form ? (
            <SimpleDnsForm errors={fieldErrors} settings={form} updateSimple={updateSimple} />
          ) : (
            <div className="text-sm text-muted-foreground">Loading DNS settings</div>
          )}
        </aside>

        <div className="min-h-0 overflow-hidden">
          {form ? (
            <Tabs className="flex h-full min-h-0 flex-col" defaultValue="xray">
              <div className="shrink-0 border-b px-4 py-2">
                <TabsList>
                  <TabsTrigger value="xray">
                    <Braces className="size-4" aria-hidden="true" />
                    Xray
                  </TabsTrigger>
                  <TabsTrigger value="singbox">
                    <ServerCog className="size-4" aria-hidden="true" />
                    sing-box
                  </TabsTrigger>
                </TabsList>
              </div>
              <TabsContent className="m-0 min-h-0 flex-1" value="xray">
                <AdvancedDnsEditor
                  defaults={{
                    normal: form.defaults.xrayNormalDns,
                    tun: form.defaults.xrayTunDns,
                  }}
                  errors={fieldErrors}
                  fieldPrefix="xrayDnsItem"
                  item={form.xrayDnsItem}
                  onChange={(patch) => updateCore("xrayDnsItem", patch)}
                  showSystemHosts
                  title="Xray raw DNS"
                />
              </TabsContent>
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
    </section>
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
    <div className="grid gap-4">
      <div className="flex items-center gap-2">
        <ShieldCheck className="size-4 text-muted-foreground" aria-hidden="true" />
        <h3 className="text-sm font-semibold">Simple DNS</h3>
      </div>

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
    </div>
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
        <label className="ms-auto flex items-center gap-2 text-sm">
          <input checked={item.Enabled} onChange={(event) => onChange({ Enabled: event.target.checked })} type="checkbox" />
          Enabled
        </label>
      </div>

      <div className="grid min-h-0 flex-1 gap-4 overflow-auto p-4 xl:grid-cols-2">
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
    <label className="grid min-h-[22rem] gap-2 text-sm">
      <span className="flex items-center gap-2 font-medium">
        <Braces className="size-4 text-muted-foreground" aria-hidden="true" />
        {label}
        <Button className="ms-auto h-7 px-2" onClick={onReset} type="button" variant="outline">
          <RotateCcw className="size-3.5" aria-hidden="true" />
          Default
        </Button>
      </span>
      <div className={cn("overflow-hidden rounded-md border", error ? "border-destructive" : "")}>
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
    </label>
  );
}

function CheckboxField({
  checked,
  disabled = false,
  label,
  onChange,
}: {
  checked: boolean;
  disabled?: boolean;
  label: string;
  onChange: (value: boolean) => void;
}) {
  return (
    <label className={cn("flex items-center gap-2 text-sm", disabled ? "opacity-55" : "")}>
      <input checked={checked} disabled={disabled} onChange={(event) => onChange(event.target.checked)} type="checkbox" />
      {label}
    </label>
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
    <label className="grid gap-1.5 text-sm">
      <span className="font-medium">{label}</span>
      <input
        className="h-9 rounded-md border bg-background px-3 outline-none focus:border-ring"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
    </label>
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
  return (
    <label className="grid gap-1.5 text-sm">
      <span className="font-medium">{label}</span>
      <select
        className="h-9 rounded-md border bg-background px-3 outline-none focus:border-ring"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        {STRATEGIES.map((strategy) => (
          <option key={strategy || "default"} value={strategy}>
            {strategy || "default"}
          </option>
        ))}
      </select>
    </label>
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
  return (
    <label className="grid gap-1.5 text-sm">
      <span className="font-medium">{label}</span>
      <textarea
        className={cn("min-h-24 resize-y rounded-md border bg-background px-3 py-2 outline-none focus:border-ring", error ? "border-destructive" : "")}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
      {error ? <span className="text-xs text-destructive">{error}</span> : null}
    </label>
  );
}
