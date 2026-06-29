import { useEffect, useMemo, useState } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { json } from "@codemirror/lang-json";
import { AlertTriangle, Braces, FileJson2, Save } from "lucide-react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useI18n } from "@/i18n/use-i18n";
import { loadFullConfigTemplates, saveFullConfigTemplate } from "@/ipc";
import type { CoreType, FullConfigTemplateItem_Serialize } from "@/ipc/bindings";
import { cn, getErrorMessage } from "@/lib/utils";
import { CORE_TYPES } from "@/features/profiles/profile-constants";

type TemplateTab = "singbox";

type TemplateForm = {
  AddProxyOnly: boolean;
  Config: string;
  CoreType: CoreType;
  Enabled: boolean;
  Id: string;
  ProxyDetour: string;
  Remarks: string;
  TunConfig: string;
};

const editorExtensions = [json()];

const templateTabs: Array<{ coreType: CoreType; label: string; value: TemplateTab }> = [
  { coreType: CORE_TYPES.singBox, label: "sing-box", value: "singbox" },
];

const emptyForms = Object.fromEntries(
  templateTabs.map((tab) => [tab.value, createEmptyForm(tab.coreType, tab.label)]),
) as Record<TemplateTab, TemplateForm>;

export function FullConfigTemplateDialog() {
  const { t } = useI18n();
  const [activeTab, setActiveTab] = useState<TemplateTab>("singbox");
  const [error, setError] = useState<string | null>(null);
  const [fieldError, setFieldError] = useState<string | null>(null);
  const [forms, setForms] = useState<Record<TemplateTab, TemplateForm>>(emptyForms);
  const [saved, setSaved] = useState<string | null>(null);
  const [working, setWorking] = useState(true);

  useEffect(() => {
    let cancelled = false;

    void loadFullConfigTemplates()
      .then((templates) => {
        if (cancelled) {
          return;
        }
        setForms(materializeForms(templates));
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

  const activeForm = forms[activeTab];
  const activeLabel = useMemo(
    () => templateTabs.find((tab) => tab.value === activeTab)?.label ?? activeTab,
    [activeTab],
  );

  function updateActiveForm(patch: Partial<TemplateForm>) {
    setSaved(null);
    setFieldError(null);
    setForms((current) => ({ ...current, [activeTab]: { ...current[activeTab], ...patch } }));
  }

  async function saveActiveTemplate() {
    setWorking(true);
    setError(null);
    setFieldError(null);
    setSaved(null);
    try {
      const config = parseOptionalJsonObject(activeForm.Config, t("templates.configJson"));
      const tunConfig = parseOptionalJsonObject(activeForm.TunConfig, t("templates.tunConfigJson"));
      const savedTemplate = await saveFullConfigTemplate({
        AddProxyOnly: activeForm.AddProxyOnly,
        Config: config,
        CoreType: activeForm.CoreType,
        Enabled: activeForm.Enabled,
        Id: activeForm.Id,
        ProxyDetour: activeForm.ProxyDetour.trim() || null,
        Remarks: activeForm.Remarks.trim() || activeLabel,
        TunConfig: tunConfig,
      });
      setForms((current) => ({ ...current, [activeTab]: toForm(savedTemplate, activeLabel) }));
      setSaved(t("templates.saved", { core: activeLabel }));
    } catch (error) {
      setFieldError(getErrorMessage(error));
    } finally {
      setWorking(false);
    }
  }

  return (
    <DialogContent className="max-h-[90dvh] w-[calc(100vw-2rem)] max-w-5xl overflow-hidden p-0">
      <DialogHeader className="pe-12 px-6 pb-4 pt-6">
        <DialogTitle className="flex items-center gap-2">
          <FileJson2 className="size-4" aria-hidden="true" />
          {t("templates.title")}
        </DialogTitle>
        <DialogDescription>{t("templates.description")}</DialogDescription>
      </DialogHeader>

      <Tabs
        className="grid max-h-[calc(90dvh-9rem)] min-h-0 gap-4 overflow-hidden px-6 pb-6"
        onValueChange={(value) => setActiveTab(value as TemplateTab)}
        value={activeTab}
      >
        <TabsList>
          {templateTabs.map((tab) => (
            <TabsTrigger key={tab.value} value={tab.value}>
              {tab.label}
            </TabsTrigger>
          ))}
        </TabsList>

        {templateTabs.map((tab) => {
          const form = forms[tab.value];

          return (
            <TabsContent className="m-0 min-h-0 overflow-y-auto" key={tab.value} value={tab.value}>
              <div className="grid gap-4">
                <div className="grid gap-3 md:grid-cols-[1fr_14rem_14rem] md:items-end">
                  <div className="grid min-w-0 gap-1">
                    <Label className="text-xs text-muted-foreground" htmlFor={`${tab.value}-template-remarks`}>
                      {t("templates.remarks")}
                    </Label>
                    <Input
                      className="bg-card"
                      id={`${tab.value}-template-remarks`}
                      onChange={(event) => updateActiveForm({ Remarks: event.currentTarget.value })}
                      value={form.Remarks}
                    />
                  </div>

                  <ToggleField
                    checked={form.Enabled}
                    label={t("templates.enabled")}
                    onChange={(Enabled) => updateActiveForm({ Enabled })}
                  />
                  <ToggleField
                    checked={form.AddProxyOnly}
                    label={t("templates.addProxyOnly")}
                    onChange={(AddProxyOnly) => updateActiveForm({ AddProxyOnly })}
                  />
                </div>

                <div className="grid gap-1">
                  <Label className="text-xs text-muted-foreground" htmlFor={`${tab.value}-template-detour`}>
                    {t("templates.proxyDetour")}
                  </Label>
                  <Input
                    className="bg-card"
                    id={`${tab.value}-template-detour`}
                    onChange={(event) => updateActiveForm({ ProxyDetour: event.currentTarget.value })}
                    value={form.ProxyDetour}
                  />
                </div>

                <div className="grid gap-4 xl:grid-cols-2">
                  <JsonEditor
                    error={fieldError}
                    label={t("templates.configJson")}
                    onChange={(Config) => updateActiveForm({ Config })}
                    value={form.Config}
                  />
                  <JsonEditor
                    label={t("templates.tunConfigJson")}
                    onChange={(TunConfig) => updateActiveForm({ TunConfig })}
                    value={form.TunConfig}
                  />
                </div>
              </div>
            </TabsContent>
          );
        })}

        {saved ? (
          <Alert role="status">
            <AlertDescription>{saved}</AlertDescription>
          </Alert>
        ) : null}
        {error || fieldError ? (
          <Alert variant="destructive">
            <AlertTriangle aria-hidden="true" />
            <AlertDescription>{error || fieldError}</AlertDescription>
          </Alert>
        ) : null}
      </Tabs>

      <DialogFooter className="border-t px-6 py-4">
        <Button disabled={working} onClick={() => void saveActiveTemplate()} type="button">
          <Save className="size-4" aria-hidden="true" />
          {working ? t("templates.saving") : t("actions.save")}
        </Button>
      </DialogFooter>
    </DialogContent>
  );
}

function JsonEditor({
  error,
  label,
  onChange,
  value,
}: {
  error?: string | null;
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  return (
    <div className="grid min-h-[22rem] gap-2">
      <Label className="flex items-center gap-2 text-xs font-medium uppercase text-muted-foreground">
        <Braces className="size-4" aria-hidden="true" />
        {label}
      </Label>
      <div
        aria-invalid={error ? true : undefined}
        className={cn(
          "overflow-hidden rounded-md border border-input bg-background shadow-xs transition-[color,box-shadow] focus-within:border-accent-blue focus-within:ring-[3px] focus-within:ring-accent-blue/40 dark:bg-input/30",
          error ? "border-destructive ring-destructive/20 dark:ring-destructive/40" : "",
        )}
      >
        <CodeMirror
          basicSetup={{ foldGutter: true, highlightActiveLine: true, lineNumbers: true }}
          extensions={editorExtensions}
          height="20rem"
          onChange={onChange}
          value={value}
        />
      </div>
    </div>
  );
}

function ToggleField({
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

function materializeForms(templates: FullConfigTemplateItem_Serialize[]): Record<TemplateTab, TemplateForm> {
  return Object.fromEntries(
    templateTabs.map((tab) => {
      const item = templates.find((template) => template.CoreType === tab.coreType);

      return [tab.value, item ? toForm(item, tab.label) : createEmptyForm(tab.coreType, tab.label)];
    }),
  ) as Record<TemplateTab, TemplateForm>;
}

function toForm(template: FullConfigTemplateItem_Serialize, fallbackLabel: string): TemplateForm {
  return {
    AddProxyOnly: template.AddProxyOnly ?? false,
    Config: formatJsonForEditor(template.Config),
    CoreType: template.CoreType,
    Enabled: template.Enabled,
    Id: template.Id,
    ProxyDetour: template.ProxyDetour ?? "",
    Remarks: template.Remarks || fallbackLabel,
    TunConfig: formatJsonForEditor(template.TunConfig),
  };
}

function createEmptyForm(coreType: CoreType, label: string): TemplateForm {
  return {
    AddProxyOnly: false,
    Config: "",
    CoreType: coreType,
    Enabled: false,
    Id: "",
    ProxyDetour: "",
    Remarks: label,
    TunConfig: "",
  };
}

function formatJsonForEditor(value?: string | null): string {
  if (!value?.trim()) {
    return "";
  }

  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}

function parseOptionalJsonObject(value: string, label: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  const parsed = JSON.parse(trimmed) as unknown;
  if (!isJsonObject(parsed)) {
    throw new Error(`${label} must be a JSON object.`);
  }

  return JSON.stringify(parsed, null, 2);
}

function isJsonObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
