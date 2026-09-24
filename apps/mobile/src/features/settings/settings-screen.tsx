import { useI18n } from "@voya/i18n/use-i18n";
import { openPage } from "~/app/navigation";
import { DetailScreen } from "~/components/detail-screen";
import { ListCard } from "~/components/list-card";
import { ListRow } from "~/components/list-row";
import { PageHeader } from "~/components/page-header";

export function SettingsScreen() {
  const { t } = useI18n();
  return <DetailScreen>
    <PageHeader title={t("tabs.settings")} />
    <ListCard>
      <ListRow title={t("mobile.general")} testID="settings-general" onPress={() => openPage("general")} />
      <ListRow title={t("mobile.connection")} testID="settings-dns" onPress={() => openPage("dns")} />
      <ListRow title={t("mobile.maintenance")} testID="settings-maintenance" onPress={() => openPage("maintenance")} />
      <ListRow last title={t("mobile.about")} testID="settings-about" onPress={() => openPage("about")} />
    </ListCard>
  </DetailScreen>;
}
