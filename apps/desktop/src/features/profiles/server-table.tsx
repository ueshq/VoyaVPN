import { PageSection, PageTitle } from "@/components/app-shell/page-section";

import { ServerTableDialogs } from "./server-table-dialogs";
import { ServerTableGrid } from "./server-table-grid";
import { ServerTableToolbar } from "./server-table-toolbar";
import { useServerTable } from "./use-server-table";

export function ProfilesScreen() {
  const controller = useServerTable();

  return (
    <PageSection aria-label={controller.t("panes.profiles.title")}>
      <PageTitle title={controller.t("panes.profiles.title")} />
      <ServerTableToolbar controller={controller} />
      <ServerTableGrid controller={controller} />
      <ServerTableDialogs controller={controller} />
    </PageSection>
  );
}
