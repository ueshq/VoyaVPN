import { PageSection, PageTitle } from "@/components/app-shell/page-section";
import { useI18n } from "@voya/i18n/use-i18n";

import { ServerTableDialogs } from "./server-table-dialogs";
import { ProfileCardList } from "./server-table-grid";
import { ServerTableToolbar } from "./server-table-toolbar";
import { useServerTable } from "./use-server-table";

export function ProfilesScreen() {
  const { t } = useI18n();
  const controller = useServerTable();
  return (
    <PageSection className="profile-cards-screen" aria-label={t("panes.profiles.title")}>
      <PageTitle title={t("panes.profiles.title")} />
      <ServerTableToolbar controller={controller} />
      <ProfileCardList controller={controller} />
      <ServerTableDialogs controller={controller} />
    </PageSection>
  );
}
