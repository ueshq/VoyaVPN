import { useState, useSyncExternalStore } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { RotateCcw } from "lucide-react";

import { PageContent, PageSection, PageTitle } from "@/components/app-shell/page-section";
import { InlinePageError } from "@/components/app-shell/inline-page-error";
import { Button } from "@voya/ui/components/button";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@voya/ui/components/tabs";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
import { DnsPane } from "@/features/dns/dns-pane";
import { useDnsSettings } from "@/features/dns/use-dns-settings";
import { UpdatesPanel } from "@/features/updates/updates-panel";
import { useShellStore, type SettingsTab } from "@/stores/shell-store";

import { AdvancedTab } from "./advanced-tab";
import { ConnectionTab } from "./connection-tab";
import { GeneralTab } from "./general-tab";
import { SettingsApplyStatus } from "./settings-apply-status";
import { SettingsFields } from "./settings-form";
import { settingsSaveQueue } from "./settings-save-queue";
import { useAppSettings, type AppSettingsController } from "./use-app-settings";

// Everyday choices first; anything that needs networking knowledge waits under
// Advanced.
const tabs = [
  { labelKey: "settings.tabGeneral", value: "general" },
  { labelKey: "settings.tabConnection", value: "connection" },
  { labelKey: "settings.tabAdvanced", value: "advanced" },
  { labelKey: "settings.tabUpdates", value: "updates" },
] as const satisfies readonly { labelKey: TranslationKey; value: SettingsTab }[];

export function SettingsScreen() {
  const { t } = useI18n();
  const controller = useAppSettings();
  const tab = useShellStore((state) => state.settingsTab);
  const [visited, setVisited] = useState<ReadonlySet<SettingsTab>>(
    () => new Set([tab]),
  );
  const dns = useDnsSettings(visited.has("connection"));
  const queue = settingsSaveQueue(useQueryClient());
  const saving = useSyncExternalStore(
    queue.subscribe,
    queue.isSaving,
    queue.isSaving,
  );
  const error = controller.error ?? dns.operationError;

  function changeTab(value: string) {
    const next = tabs.find((item) => item.value === value)?.value;
    if (!next) return;
    if (
      document.activeElement instanceof HTMLInputElement ||
      document.activeElement instanceof HTMLTextAreaElement
    )
      document.activeElement.blur();
    useShellStore.setState({ settingsTab: next });
    setVisited((current) => new Set(current).add(next));
  }

  return (
    <PageSection aria-label={t("modal.settings")}>
      <Tabs
        className="flex min-h-0 flex-1 flex-col gap-0"
        value={tab}
        onValueChange={changeTab}
      >
        <PageTitle
          actions={
            <TabsList aria-label={t("settings.categories")}>
              {tabs.map((item) => (
                <TabsTrigger key={item.value} value={item.value}>
                  {t(item.labelKey)}
                </TabsTrigger>
              ))}
            </TabsList>
          }
          title={t("modal.settings")}
        />
        <PageContent>
          <SettingsApplyStatus
            failed={!!error}
            saved={controller.saved || dns.saved}
            saving={saving}
          />
          {error ? (
            <InlinePageError>
              <span>{error}</span>
              <Button
                className="ms-auto"
                disabled={saving}
                onClick={() => {
                  controller.retry();
                  dns.retry();
                }}
                size="sm"
                variant="outline"
              >
                <RotateCcw aria-hidden="true" className="size-3.5" />
                {t("settings.autosave.retry")}
              </Button>
            </InlinePageError>
          ) : null}
          <div className="min-h-0 min-w-0 flex-1">
            <SettingsFields errors={controller.fieldErrors}>
              {tabs.map((item) => (
                <TabsContent
                  key={item.value}
                  className="h-full overflow-y-auto data-[state=inactive]:hidden"
                  forceMount
                  value={item.value}
                >
                  {!visited.has(item.value) ? null : item.value === "updates" ? (
                    <UpdatesPanel />
                  ) : (
                    <AppSettingsPane
                      controller={controller}
                      dns={dns}
                      tab={item.value}
                    />
                  )}
                </TabsContent>
              ))}
            </SettingsFields>
          </div>
        </PageContent>
      </Tabs>
    </PageSection>
  );
}

function AppSettingsPane({
  controller,
  dns,
  tab,
}: {
  controller: AppSettingsController;
  dns: ReturnType<typeof useDnsSettings>;
  tab: Exclude<SettingsTab, "updates">;
}) {
  const { t } = useI18n();
  if (!controller.settings)
    return (
      <p className="text-sm text-muted-foreground" role="status">
        {controller.working ? t("options.loading") : null}
      </p>
    );
  const ready = { ...controller, settings: controller.settings };
  switch (tab) {
    case "general":
      return <GeneralTab controller={ready} />;
    case "connection":
      return (
        <div className="grid gap-4">
          <ConnectionTab controller={ready} />
          <DnsPane controller={dns} />
        </div>
      );
    case "advanced":
      return <AdvancedTab controller={ready} />;
  }
}
