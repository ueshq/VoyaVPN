import { useState } from "react";
import { PageContent, PageSection, PageSurface, PageTitle } from "@/components/app-shell/page-section";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@voya/ui/components/tabs";
import { useI18n } from "@voya/i18n/use-i18n";
import { useShellStore } from "@/stores/shell-store";

import { ConnectionsPanel } from "./connections-panel";
import { ProxyGroupsPanel } from "./proxy-groups-panel";

/**
 * The Connections shell destination: one large page title with two sub-views —
 * the live connection table and the running policy group. The runtime log lives
 * under Settings → Advanced. The sub-view lives in the shell store so it
 * survives leaving the page and stays deep-linkable from backend `selectTab`
 * events. Inactive panels unmount (one heavy virtualizer at a time).
 */
export function ConnectionsScreen() {
  const { t } = useI18n();
  const view = useShellStore((state) => state.connectionsView);
  const setView = useShellStore((state) => state.setConnectionsView);
  const [connectionSearch, setConnectionSearch] = useState("");

  return (
    <PageSection aria-label={t("tabs.connections")}>
      <Tabs
        className="flex min-h-0 flex-1 flex-col gap-0"
        onValueChange={(value) => {
          if (value === "connections" || value === "proxies") {
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
          </PageSurface>
        </PageContent>
      </Tabs>
    </PageSection>
  );
}
