import { useState } from "react";
import { Cpu, Database, Gauge, Globe2, Network, RefreshCw, Settings2, type LucideIcon } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { Tabs, TabsContent } from "@voya/ui/components/tabs";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
import { DnsPane } from "@/features/dns/dns-pane";
import { UpdatesPanel } from "@/features/updates";

import { CoreTab } from "./core-tab";
import { GeneralTab } from "./general-tab";
import { NetworkTab } from "./network-tab";
import {
  discardSettingsDirtySources,
  saveSettingsDirtySources,
  useSettingsDirtySources,
} from "./settings-dirty-sources";
import { SettingsTabBar, SettingsTabTrigger } from "./settings-tabs";
import { SourcesTab } from "./sources-tab";
import { TestsTab } from "./tests-tab";
import {
  useAppSettings,
  type AppSettingsController,
} from "./use-app-settings";

export type SettingsTab = "core" | "dns" | "general" | "network" | "sources" | "tests" | "updates";

const tabDefs: Array<{ icon: LucideIcon; labelKey: TranslationKey; value: SettingsTab }> = [
  { icon: Settings2, labelKey: "settings.tabGeneral", value: "general" },
  { icon: Globe2, labelKey: "options.sources", value: "sources" },
  { icon: Cpu, labelKey: "options.runtimeCore", value: "core" },
  { icon: Network, labelKey: "options.runtimeNetwork", value: "network" },
  { icon: Database, labelKey: "tabs.dns", value: "dns" },
  { icon: Gauge, labelKey: "settings.tabTests", value: "tests" },
  { icon: RefreshCw, labelKey: "settings.tabUpdates", value: "updates" },
];

const tabValues = new Set<SettingsTab>(tabDefs.map((def) => def.value));

export function SettingsSurface({
  controller,
  initialTab = "general",
}: {
  controller?: AppSettingsController;
  initialTab?: SettingsTab;
}) {
  if (controller) {
    return <SettingsSurfaceView controller={controller} initialTab={initialTab} />;
  }
  return <OwnedSettingsSurface initialTab={initialTab} />;
}

function OwnedSettingsSurface({ initialTab }: { initialTab: SettingsTab }) {
  const controller = useAppSettings();
  return <SettingsSurfaceView controller={controller} initialTab={initialTab} />;
}

function SettingsSurfaceView({
  controller,
  initialTab,
}: {
  controller: AppSettingsController;
  initialTab: SettingsTab;
}) {
  const { direction, t } = useI18n();
  const [tab, setTab] = useState<SettingsTab>(initialTab);
  const [visited, setVisited] = useState<ReadonlySet<SettingsTab>>(() => new Set([initialTab]));
  // Panes with their own draft (the DNS tab) report through the registry, so a
  // single footer speaks for every unsaved edit on the surface.
  const paneDirty = useSettingsDirtySources();
  const [paneError, setPaneError] = useState<string | null>(null);
  const dirty = controller.dirty || paneDirty;
  const footerError = controller.error ?? paneError;

  function handleTabChange(value: string) {
    if (!tabValues.has(value as SettingsTab)) return;
    const next = value as SettingsTab;
    setTab(next);
    setVisited((current) => (current.has(next) ? current : new Set(current).add(next)));
  }

  async function saveAll() {
    setPaneError(null);
    if (controller.dirty && !(await controller.save())) return;
    setPaneError(await saveSettingsDirtySources());
  }

  async function discardAll() {
    setPaneError(null);
    if (controller.dirty) await controller.discard();
    await discardSettingsDirtySources();
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <Tabs className="flex min-h-0 flex-1 flex-col gap-0" dir={direction} onValueChange={handleTabChange} value={tab}>
        <SettingsTabBar>
          {tabDefs.map((def) => (
            <SettingsTabTrigger key={def.value} icon={def.icon} label={t(def.labelKey)} value={def.value} />
          ))}
        </SettingsTabBar>
        <div className="min-h-0 flex-1">
          {tabDefs.map((def) => (
            <TabsContent key={def.value} className="h-full overflow-y-auto px-8 py-6 data-[state=inactive]:hidden" forceMount value={def.value}>
              {visited.has(def.value) ? <SettingsPane controller={controller} tab={def.value} /> : null}
            </TabsContent>
          ))}
        </div>
      </Tabs>

      <footer className="flex shrink-0 flex-wrap items-center gap-3 border-t bg-background px-8 py-3">
        <Button disabled={!dirty || controller.working} onClick={() => void saveAll()} type="button">
          {t("settings.saveAll")}
        </Button>
        <Button disabled={!dirty || controller.working} onClick={() => void discardAll()} type="button" variant="outline">
          {t("settings.discardChanges")}
        </Button>
        <span className="text-xs text-muted-foreground">
          {dirty ? t("settings.unsavedChanges") : controller.saved ? t("options.saved") : null}
        </span>
        {footerError ? (
          <span className="text-xs text-destructive" role="alert">{footerError}</span>
        ) : null}
      </footer>
    </div>
  );
}

function SettingsPane({
  controller,
  tab,
}: {
  controller: AppSettingsController;
  tab: SettingsTab;
}) {
  const { t } = useI18n();
  switch (tab) {
    case "general":
      return <GeneralTab controller={controller} />;
    case "sources":
      return <SourcesTab controller={controller} />;
    case "core":
      return <CoreTab controller={controller} />;
    case "network":
      return <NetworkTab controller={controller} />;
    case "dns":
      return <DnsPane />;
    case "tests":
      return <TestsTab controller={controller} />;
    case "updates":
      return (
        <fieldset className="grid gap-2" disabled={controller.dirty}>
          {controller.dirty ? <p className="text-xs text-muted-foreground">{t("settings.saveBeforeActions")}</p> : null}
          <UpdatesPanel />
        </fieldset>
      );
  }
}
