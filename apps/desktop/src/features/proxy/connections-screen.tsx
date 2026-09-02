import { PageSection, PageTitle } from "@/components/app-shell/page-section";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@voya/ui/components/tabs";
import { useI18n } from "@voya/i18n/use-i18n";
import { useShellStore } from "@/stores/shell-store";

import { ConnectionsPanel } from "./connections-panel";
import { LogsPanel } from "@/features/logs/logs-panel";

/**
 * The Connections shell destination: one large page title with two sub-views —
 * the live connection table and the log tail. The sub-view lives in the shell
 * store so it survives leaving the page and stays deep-linkable from backend
 * `selectTab` events. Inactive panels unmount (one heavy virtualizer at a time).
 */
export function ConnectionsScreen() {
  const { direction, t } = useI18n();
  const view = useShellStore((state) => state.connectionsView);
  const setView = useShellStore((state) => state.setConnectionsView);

  return (
    <PageSection aria-label={t("tabs.connections")}>
      <Tabs
        className="flex min-h-0 flex-1 flex-col gap-0"
        dir={direction}
        onValueChange={(value) => {
          if (value === "connections" || value === "logs") {
            setView(value);
          }
        }}
        value={view}
      >
        <PageTitle
          actions={
            <TabsList>
              <TabsTrigger value="connections">{t("tabs.connections")}</TabsTrigger>
              <TabsTrigger value="logs">{t("tabs.logs")}</TabsTrigger>
            </TabsList>
          }
          title={t("tabs.connections")}
        />
        <TabsContent className="min-h-0 flex-1" value="connections">
          <ConnectionsPanel />
        </TabsContent>
        <TabsContent className="min-h-0 flex-1" value="logs">
          <LogsPanel />
        </TabsContent>
      </Tabs>
    </PageSection>
  );
}
