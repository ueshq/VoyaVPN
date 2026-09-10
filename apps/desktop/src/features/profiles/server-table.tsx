import { lazy, Suspense } from "react";
import { PageSection, PageTitle } from "@/components/app-shell/page-section";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@voya/ui/components/tabs";
import { useI18n } from "@voya/i18n/use-i18n";
import { useShellStore } from "@/stores/shell-store";

import { ServerTableDialogs } from "./server-table-dialogs";
import { ProfileCardList } from "./server-table-grid";
import { ServerTableToolbar } from "./server-table-toolbar";
import { useServerTable } from "./use-server-table";

const ProxyGroupsPanel = lazy(() => import("@/features/proxy/proxy-groups-screen").then((module) => ({
  default: module.ProxyGroupsPanel,
})));

export function ProfilesScreen() {
  const { t } = useI18n();
  const view = useShellStore((state) => state.profilesView);
  const setView = useShellStore((state) => state.setProfilesView);
  return (
    <PageSection className="profile-cards-screen" aria-label={t("panes.profiles.title")}>
      <Tabs className="flex min-h-0 flex-1 flex-col gap-0" value={view} onValueChange={(value) => {
        if (value === "profiles" || value === "proxyGroups") setView(value);
      }}>
        <PageTitle title={t("panes.profiles.title")} actions={
          <TabsList aria-label={t("panes.profiles.viewTabsAria")}>
            <TabsTrigger value="profiles">{t("tabs.profiles")}</TabsTrigger>
            <TabsTrigger value="proxyGroups">{t("panes.proxyGroups.title")}</TabsTrigger>
          </TabsList>
        } />
        <TabsContent className="min-h-0 flex-1" value="profiles"><ProfilesPanel /></TabsContent>
        <TabsContent className="min-h-0 flex-1" value="proxyGroups">
          <Suspense fallback={<div aria-label={t("status.loadingScreen")} className="h-full animate-pulse bg-surface-raised/40" />}>
            <ProxyGroupsPanel />
          </Suspense>
        </TabsContent>
      </Tabs>
    </PageSection>
  );
}

function ProfilesPanel() {
  const controller = useServerTable();
  return (
    <div className="flex h-full min-h-0 flex-col">
      <ServerTableToolbar controller={controller} />
      <ProfileCardList controller={controller} />
      <ServerTableDialogs controller={controller} />
    </div>
  );
}
