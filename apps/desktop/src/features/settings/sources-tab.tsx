import { useState } from "react";
import { Download } from "lucide-react";

import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@voya/ui/components/dialog";
import { Input } from "@voya/ui/components/input";
import { cn } from "@voya/ui/lib/utils";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
import { importConfigTemplate } from "@/ipc";
import type { ConfigSourceSettings, ConfigTemplateSelection, SourceSettings } from "@/ipc/bindings";
import { useToastStore } from "@/stores/toast-store";
import { getErrorMessage } from "@voya/utils/error";

import { SettingsGroup, SettingsRow } from "./settings-form";
import type { AppSettingsController } from "./use-app-settings";

type SourceForm = {
  geoSourceUrl: string;
  routeRulesTemplateSourceUrl: string;
  srsSourceUrl: string;
};

type TemplateType = "default" | "custom";

const templateOptions: Array<{
  descriptionKey: TranslationKey;
  labelKey: TranslationKey;
  type: TemplateType;
}> = [
  { descriptionKey: "options.configTemplate.defaultDescription", labelKey: "options.configTemplate.default", type: "default" },
  { descriptionKey: "options.configTemplate.customDescription", labelKey: "options.configTemplate.custom", type: "custom" },
];

export function SourcesTab({ controller }: { controller: AppSettingsController }) {
  const { t } = useI18n();
  const pushToast = useToastStore((state) => state.pushToast);
  const [importError, setImportError] = useState<string | null>(null);
  const [importOpen, setImportOpen] = useState(false);
  const [importWorking, setImportWorking] = useState(false);
  const [selectedTemplate, setSelectedTemplate] = useState<TemplateType | null>(null);
  const { settings, dirty, update, working } = controller;

  if (!settings) {
    return <p className="text-xs text-muted-foreground">{working ? t("options.loading") : controller.error}</p>;
  }

  const form = toSourceForm(settings.sources);
  const patchSources = (patch: Partial<SourceForm>) => {
    const next = { ...form, ...patch };
    update((current) => ({
      ...current,
      sources: sourceSettingsFromForm(next, current.sources.subscriptionConverter),
    }));
  };

  function handleImportOpenChange(open: boolean) {
    if (importWorking) return;
    setImportOpen(open);
    if (!open) {
      setImportError(null);
      setSelectedTemplate(null);
    }
  }

  async function applyTemplate() {
    if (!selectedTemplate || dirty) return;
    if (selectedTemplate === "custom") {
      const validationError = validateCustomSources(form);
      if (validationError) {
        setImportError(t(validationError));
        return;
      }
    }

    const selection: ConfigTemplateSelection =
      selectedTemplate === "custom"
        ? { sources: configSourcesFromForm(form), type: "custom" }
        : { type: selectedTemplate };
    setImportWorking(true);
    setImportError(null);
    try {
      // `import_config_template` emits dns + routings + appSettings; the
      // reload is still needed because this tab edits a settings draft that
      // an invalidation alone would not re-seed.
      const result = await importConfigTemplate(selection);
      await controller.reload();
      const descriptions = [t("options.configTemplate.appliedDescription")];
      if (result.reusedExistingRouting) descriptions.push(t("options.configTemplate.reusedDescription"));
      pushToast({
        description: descriptions.join(" "),
        severity: "info",
        title: t("options.configTemplate.applied"),
      });
      setImportOpen(false);
      setSelectedTemplate(null);
    } catch (error) {
      setImportError(getErrorMessage(error));
    } finally {
      setImportWorking(false);
    }
  }

  return (
    <div className="grid gap-4">
      <SettingsGroup>
        <SourceField disabled={working} id="ruleset-geo-source-url" label={t("options.geoSource")} onChange={(geoSourceUrl) => patchSources({ geoSourceUrl })} value={form.geoSourceUrl} />
        <SourceField disabled={working} id="ruleset-srs-source-url" label={t("options.srsSource")} onChange={(srsSourceUrl) => patchSources({ srsSourceUrl })} value={form.srsSourceUrl} />
        <SourceField disabled={working} id="routing-template-source-url" label={t("options.routeTemplateSource")} onChange={(routeRulesTemplateSourceUrl) => patchSources({ routeRulesTemplateSourceUrl })} value={form.routeRulesTemplateSourceUrl} />
        <SourceField
          disabled={working}
          id="subscription-convert-url"
          label={t("settings.sources.subscriptionConverter")}
          onChange={(subConvertUrl) =>
            update((current) => ({
              ...current,
              sources: {
                ...current.sources,
                subscriptionConverter: subConvertUrl.trim() || null,
              },
            }))
          }
          value={settings.sources.subscriptionConverter ?? ""}
        />
        <SettingsRow>
          <Button
            disabled={working || dirty}
            onClick={() => {
              setImportError(null);
              setSelectedTemplate(null);
              setImportOpen(true);
            }}
            size="sm"
            type="button"
            variant="outline"
          >
            <Download className="size-4" aria-hidden="true" />
            {t("options.configTemplate.import")}
          </Button>
          {dirty ? <p className="text-xs text-muted-foreground">{t("settings.saveBeforeActions")}</p> : null}
        </SettingsRow>
      </SettingsGroup>

      <Dialog open={importOpen} onOpenChange={handleImportOpenChange}>
        <DialogContent className="max-w-2xl" closeLabel={t("actions.close")}>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Download className="size-4" aria-hidden="true" />
              {t("options.configTemplate.title")}
            </DialogTitle>
            <DialogDescription>{t("options.configTemplate.selectPrompt")}</DialogDescription>
          </DialogHeader>
          <div className="grid gap-3 px-6 py-4">
            <p className="text-sm text-muted-foreground">{t("options.configTemplate.advancedHint")}</p>
            <div className="grid gap-2 sm:grid-cols-2">
              {templateOptions.map((option) => {
                const selected = selectedTemplate === option.type;
                return (
                  <Button
                    key={option.type}
                    aria-pressed={selected}
                    className={cn(
                      "h-auto min-h-20 min-w-0 items-start justify-start whitespace-normal px-3 py-3 text-start",
                      selected && "border-primary bg-accent-blue-light text-brand hover:bg-accent-blue-light",
                    )}
                    disabled={importWorking}
                    onClick={() => {
                      setImportError(null);
                      setSelectedTemplate(option.type);
                    }}
                    type="button"
                    variant="outline"
                  >
                    <span className="grid min-w-0 gap-1">
                      <span className="font-medium">{t(option.labelKey)}</span>
                      <span className="text-xs font-normal text-muted-foreground">{t(option.descriptionKey)}</span>
                    </span>
                  </Button>
                );
              })}
            </div>
            {importError ? <Alert variant="destructive"><AlertDescription>{importError}</AlertDescription></Alert> : null}
          </div>
          <DialogFooter>
            <Button disabled={importWorking} onClick={() => handleImportOpenChange(false)} type="button" variant="outline">{t("actions.close")}</Button>
            <Button disabled={!selectedTemplate || importWorking} onClick={() => void applyTemplate()} type="button">
              {importWorking ? t("options.configTemplate.applying") : t("options.configTemplate.apply")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function SourceField({ disabled, id, label, onChange, value }: { disabled: boolean; id: string; label: string; onChange: (value: string) => void; value: string }) {
  return (
    <SettingsRow htmlFor={id} label={label}>
      <Input className="h-8 w-full max-w-md" disabled={disabled} id={id} onChange={(event) => onChange(event.currentTarget.value)} value={value} />
    </SettingsRow>
  );
}

function toSourceForm(settings: SourceSettings): SourceForm {
  return {
    geoSourceUrl: settings.geo ?? "",
    routeRulesTemplateSourceUrl: settings.routingTemplate ?? "",
    srsSourceUrl: settings.singboxRuleset ?? "",
  };
}

function sourceSettingsFromForm(
  form: SourceForm,
  subscriptionConverter: string | null,
): SourceSettings {
  return {
    subscriptionConverter,
    geo: form.geoSourceUrl.trim() || null,
    routingTemplate: form.routeRulesTemplateSourceUrl.trim() || null,
    singboxRuleset: form.srsSourceUrl.trim() || null,
  };
}

function configSourcesFromForm(form: SourceForm): ConfigSourceSettings {
  return {
    geoSourceUrl: form.geoSourceUrl.trim() || null,
    routeRulesTemplateSourceUrl: form.routeRulesTemplateSourceUrl.trim() || null,
    srsSourceUrl: form.srsSourceUrl.trim() || null,
  };
}

function validateCustomSources(form: SourceForm) {
  const sources = [form.geoSourceUrl, form.srsSourceUrl, form.routeRulesTemplateSourceUrl];
  if (!form.routeRulesTemplateSourceUrl.trim()) return "options.configTemplate.customSourcesRequired";
  for (const source of sources) {
    if (!source.trim()) continue;
    try {
      const parsed = new URL(source.trim());
      if (parsed.protocol !== "https:" || !parsed.hostname || parsed.username || parsed.password) {
        return "options.configTemplate.invalidSourceUrl";
      }
    } catch {
      return "options.configTemplate.invalidSourceUrl";
    }
  }
  return null;
}
