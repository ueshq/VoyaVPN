import { PageContent, PageSection, PageTitle } from "@/components/app-shell/page-section";
import { useI18n } from "@voya/i18n/use-i18n";

import { ServerTableDialogs } from "./server-table-dialogs";
import { ProfileCardList } from "./server-table-grid";
import { ServerTableToolbar } from "./server-table-toolbar";
import { ServerTableNotices } from "./server-table-notices";
import { useServerTable } from "./use-server-table";

export function ProfilesScreen() {
  const { t } = useI18n();
  const controller = useServerTable();
  return (
    <PageSection className="profile-cards-screen" aria-label={t("panes.profiles.title")}>
      <PageTitle
        title={t("panes.profiles.title")}
        actions={<ServerTableToolbar controller={controller} />}
      />
      <PageContent>
        <ServerTableNotices controller={controller} />
        <ProfileCardList controller={controller} />
      </PageContent>
      <ServerTableDialogs controller={controller} />
    </PageSection>
  );
}
