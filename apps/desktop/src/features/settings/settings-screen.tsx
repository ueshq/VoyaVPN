import { useEffect, useLayoutEffect, useRef, useState, type ComponentProps } from "react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@voya/ui/components/alert-dialog";
import { PageSection, PageTitle } from "@/components/app-shell/page-section";
import { useI18n } from "@voya/i18n/use-i18n";
import { useShellStore } from "@/stores/shell-store";

import {
  discardSettingsDirtySources,
  saveSettingsDirtySources,
  useSettingsDirtySources,
  isSettingsWorking,
  runSettingsOperation,
  useSettingsWorking,
} from "./settings-dirty-sources";
import { useAppSettings, type AppSettingsController } from "./use-app-settings";

import { Cpu, Database, Gauge, Globe2, Network, RefreshCw, Settings2, type LucideIcon } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@voya/ui/components/tabs";
import type { TranslationKey } from "@voya/i18n";
import { DnsPane } from "@/features/dns/dns-pane";
import { UpdatesPanel } from "@/features/updates/updates-panel";

import { CoreTab } from "./core-tab";
import { GeneralTab } from "./general-tab";
import { NetworkTab } from "./network-tab";
import { SourcesTab } from "./sources-tab";
import { TestsTab } from "./tests-tab";

import { cn } from "@voya/ui/lib/utils";

type SettingsTab = "core" | "dns" | "general" | "network" | "sources" | "tests" | "updates";

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

/**
 * The in-shell Settings destination. Owns the app-settings controller and a
 * navigation leave guard: switching to another shell tab while any settings
 * draft is dirty parks the target in `pendingTab` and asks to save or discard
 * first, mirroring the close guard of the former dedicated settings window.
 * Panes that keep their own draft (the DNS tab) are covered through the dirty
 * source registry — leaving the tab unmounts them, so an unguarded switch
 * would discard their edits silently.
 */
export function SettingsScreen() {
  const { t } = useI18n();
  const controller = useAppSettings();
  const paneDirty = useSettingsDirtySources();
  const pendingTab = useShellStore((state) => state.pendingTab);
  const [paneError, setPaneError] = useState<string | null>(null);
  const dirty = controller.dirty || paneDirty;
  const operationPending = useSettingsWorking();
  const working = operationPending || controller.working;
  const dialogError = controller.error ?? paneError;
  const dirtyRef = useRef(dirty);

  useLayoutEffect(() => {
    dirtyRef.current = dirty;
  }, [dirty]);

  useEffect(() => {
    useShellStore.getState().setNavigationGuard(() => !dirtyRef.current && !isSettingsWorking());
    return () => {
      useShellStore.getState().setNavigationGuard(null);
      useShellStore.getState().clearPendingTab();
    };
  }, []);

  function resolvePendingNavigation() {
    const target = useShellStore.getState().pendingTab;
    if (target) {
      useShellStore.getState().setActiveTab(target);
    }
  }

  async function saveAll(): Promise<boolean> {
    return (await runSettingsOperation(async () => {
      setPaneError(null);
      if (controller.dirty && !(await controller.save())) {
        return false;
      }
      const error = await saveSettingsDirtySources();
      setPaneError(error);
      return error === null;
    })) ?? false;
  }

  async function discardAll() {
    return runSettingsOperation(async () => {
      setPaneError(null);
      if (controller.dirty) {
        await controller.discard();
      }
      await discardSettingsDirtySources();
      return true;
    });
  }

  return (
    <PageSection aria-label={t("modal.settings")}>
      <PageTitle title={t("modal.settings")} />
      <SettingsSurface
        controller={controller}
        dirty={dirty}
        error={dialogError}
        onSave={saveAll}
        onDiscard={discardAll}
        working={working}
      />

      <AlertDialog
        onOpenChange={(open) => {
          if (!open) useShellStore.getState().clearPendingTab();
        }}
        open={pendingTab !== null}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("settings.closeUnsavedTitle")}</AlertDialogTitle>
            <AlertDialogDescription>{t("settings.closeUnsavedDescription")}</AlertDialogDescription>
          </AlertDialogHeader>
          {/* Save failures must surface inside the dialog — the surface footer's
              error line and the DNS pane's own alert sit behind the modal
              overlay where they cannot be seen. */}
          {dialogError ? (
            <p className="text-xs text-destructive" role="alert">
              {dialogError}
            </p>
          ) : null}
          <AlertDialogFooter>
            <AlertDialogCancel disabled={working}>{t("confirm.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              disabled={working}
              onClick={(event) => {
                event.preventDefault();
                void discardAll().then((discarded) => {
                  if (discarded) resolvePendingNavigation();
                });
              }}
            >
              {t("settings.discardChanges")}
            </AlertDialogAction>
            <AlertDialogAction
              disabled={working}
              onClick={(event) => {
                event.preventDefault();
                void saveAll().then((saved) => {
                  if (saved) resolvePendingNavigation();
                });
              }}
            >
              {t("settings.saveAll")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </PageSection>
  );
}

function SettingsSurface({ controller, dirty, error, onSave, onDiscard, working }: {
  controller: AppSettingsController;
  dirty: boolean;
  error: string | null;
  onSave: () => Promise<boolean>;
  onDiscard: () => Promise<boolean | undefined>;
  working: boolean;
}) {
  const { t } = useI18n();
  const [tab, setTab] = useState<SettingsTab>("general");
  const [visited, setVisited] = useState<ReadonlySet<SettingsTab>>(() => new Set(["general"]));

  function handleTabChange(value: string) {
    if (!tabValues.has(value as SettingsTab)) return;
    const next = value as SettingsTab;
    setTab(next);
    setVisited((current) => (current.has(next) ? current : new Set(current).add(next)));
  }

  return (
    <fieldset disabled={working} aria-busy={working || undefined} className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      <Tabs className="flex min-h-0 flex-1 flex-col gap-0" onValueChange={handleTabChange} value={tab}>
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
        <Button disabled={!dirty || working} onClick={() => void onSave()} type="button">
          {t("settings.saveAll")}
        </Button>
        <Button disabled={!dirty || working} onClick={() => void onDiscard()} type="button" variant="outline">
          {t("settings.discardChanges")}
        </Button>
        <span className="text-xs text-muted-foreground">
          {dirty ? t("settings.unsavedChanges") : controller.saved ? t("options.saved") : null}
        </span>
        {error ? (
          <span className="text-xs text-destructive" role="alert">{error}</span>
        ) : null}
      </footer>
    </fieldset>
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

// macOS-toolbar-style preference tabs: icon above an 11px label, the active
// tab filled with a neutral overlay tint (never the blue accent — that stays
// reserved for pressed option buttons inside the panes).
function SettingsTabBar({ className, ...props }: ComponentProps<typeof TabsList>) {
  return (
    <TabsList
      className={cn(
        "h-auto w-full flex-wrap justify-center gap-1 rounded-none border-b bg-surface-sunken px-12 py-2",
        className,
      )}
      {...props}
    />
  );
}

function SettingsTabTrigger({
  className,
  icon: Icon,
  label,
  ...props
}: Omit<ComponentProps<typeof TabsTrigger>, "children"> & {
  icon: LucideIcon;
  label: string;
}) {
  return (
    <TabsTrigger
      className={cn(
        "h-auto flex-none flex-col gap-1 rounded-md border-transparent px-3 py-1.5 text-[11px] font-medium text-muted-foreground shadow-none data-[state=active]:bg-overlay-hovered data-[state=active]:text-foreground data-[state=active]:shadow-none dark:data-[state=active]:border-transparent dark:data-[state=active]:bg-overlay-hovered",
        className,
      )}
      {...props}
    >
      <Icon className="size-5" aria-hidden="true" />
      {label}
    </TabsTrigger>
  );
}
