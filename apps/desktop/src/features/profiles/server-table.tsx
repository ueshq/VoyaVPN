import { PageContent, PageSection, PageTitle } from "@/components/app-shell/page-section";
import { useI18n } from "@voya/i18n/use-i18n";
import { Search, X } from "lucide-react";
import { Input } from "@voya/ui/components/input";
import { Button } from "@voya/ui/components/button";

import { ServerTableDialogs } from "./server-table-dialogs";
import { PolicyGroupsSection } from "./policy-groups-section";
import { ProfileCardList } from "./server-table-grid";
import { ServerTableToolbar } from "./server-table-toolbar";
import { ServerTableNotices } from "./server-table-notices";
import { useServerTable } from "./use-server-table";

export function ProfilesScreen() {
  const { t } = useI18n();
  const controller = useServerTable();
  const { search, searchRef, setSearch, clearSearch } = controller.nodeGroups;
  return (
    <PageSection className="profile-cards-screen" aria-label={t("panes.profiles.title")}>
      <PageTitle
        title={t("panes.profiles.title")}
        actions={<ServerTableToolbar controller={controller} />}
      />
      <PageContent>
        <ServerTableNotices controller={controller} />
        {controller.profiles.length > 0 || search ? (
          <div className="flex shrink-0 flex-wrap items-center gap-3">
            <div className="relative min-w-0 max-w-md flex-1">
              <Search aria-hidden="true" className="pointer-events-none absolute start-3 top-2.5 size-4 text-muted-foreground" />
              <Input
                ref={searchRef}
                aria-label={t("panes.profiles.search.placeholder")}
                placeholder={t("panes.profiles.search.placeholder")}
                className="ps-9 pe-10"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Escape") { event.stopPropagation(); setSearch(""); }
                }}
              />
              {search ? <Button
                aria-label={t("panes.profiles.search.clear")}
                className="absolute end-0.5 top-0.5 size-8"
                size="icon-sm"
                variant="ghost"
                onClick={clearSearch}
              ><X aria-hidden="true" className="size-4" /></Button> : null}
            </div>
            <p className="text-xs text-muted-foreground" role="status">
              {t("panes.profiles.search.count", { count: controller.visibleProfileCount, total: controller.profiles.length })}
            </p>
          </div>
        ) : null}
        <PolicyGroupsSection controller={controller} />
        <ProfileCardList controller={controller} />
      </PageContent>
      <ServerTableDialogs controller={controller} />
    </PageSection>
  );
}
