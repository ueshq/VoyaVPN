import { useState } from "react";
import { PageContent, PageSection, PageSurface, PageTitle } from "@/components/app-shell/page-section";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@voya/ui/components/tabs";
import { useI18n } from "@voya/i18n/use-i18n";
import { useShellStore } from "@/stores/shell-store";

import { ConnectionsPanel } from "./connections-panel";
import { ProxyGroupsPanel } from "./proxy-groups-panel";
import { LogsPanel, type LogFilter } from "@/features/logs/logs-panel";

/**
 * The Connections shell destination: one large page title with three sub-views —
 * the live connection table, the running policy group, and the log tail. The sub-view lives in the shell
 * store so it survives leaving the page and stays deep-linkable from backend
 * `selectTab` events. Inactive panels unmount (one heavy virtualizer at a time).
 */
export function ConnectionsScreen() {
  const { t } = useI18n();
  const view = useShellStore((state) => state.connectionsView);
  const setView = useShellStore((state) => state.setConnectionsView);
  const [connectionSearch, setConnectionSearch] = useState("");
  const [logSearch, setLogSearch] = useState("");
  const [logFilter, setLogFilter] = useState<LogFilter>("standard");

  return (
    <PageSection aria-label={t("tabs.connections")}>
      <Tabs
        className="flex min-h-0 flex-1 flex-col gap-0"
        onValueChange={(value) => {
          if (value === "connections" || value === "proxies" || value === "logs") {
            setView(value);
          }
        }}
        value={view}
      >
        <PageTitle
          actions={
            <TabsList aria-label={t("proxy.viewTabsAria")}>
              <TabsTrigger value="connections">{t("activity.liveConnections")}</TabsTrigger>
              <TabsTrigger value="proxies">{t("proxy.groups.tab")}</TabsTrigger>
              <TabsTrigger value="logs">{t("tabs.logs")}</TabsTrigger>
            </TabsList>
          }
          title={t("tabs.connections")}
        />
        <PageContent>
          <PageSurface className="flex flex-1 flex-col overflow-hidden">
            <TabsContent className="min-h-0 min-w-0 flex-1" value="connections">
              <ConnectionsPanel filter={connectionSearch} onFilterChange={setConnectionSearch} />
            </TabsContent>
            <TabsContent className="min-h-0 min-w-0 flex-1" value="proxies">
              <ProxyGroupsPanel />
            </TabsContent>
            <TabsContent className="min-h-0 min-w-0 flex-1" value="logs">
              <LogsPanel
                search={logSearch}
                onSearchChange={setLogSearch}
                filter={logFilter}
                onFilterChange={setLogFilter}
              />
            </TabsContent>
          </PageSurface>
        </PageContent>
      </Tabs>
    </PageSection>
  );
}
