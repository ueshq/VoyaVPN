import { PageContent, PageSection, PageTitle } from "@/components/app-shell/page-section";
import { useI18n } from "@voya/i18n/use-i18n";
import { SearchInput } from "@voya/ui/components/search-input";

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
            <SearchInput
              className="max-w-md"
              clearLabel={t("panes.profiles.search.clear")}
              label={t("panes.profiles.search.placeholder")}
              ref={searchRef}
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              onClear={clearSearch}
            />
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
