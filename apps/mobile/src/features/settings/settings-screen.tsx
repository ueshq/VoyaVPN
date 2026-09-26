import { useI18n } from "@voya/i18n/use-i18n";
import { ListGroup } from "heroui-native/list-group";
import { openPage } from "~/app/navigation";
import { DetailScreen } from "~/components/detail-screen";
import { ListRow } from "~/components/list-row";
import { PageHeader } from "~/components/page-header";

export function SettingsScreen() {
  const { t } = useI18n();
  return <DetailScreen>
    <PageHeader title={t("tabs.settings")} />
    <ListGroup>
      <ListRow chevron title={t("mobile.subscriptions")} testID="settings-subscriptions" onPress={() => openPage("subscriptions")} />
      <ListRow chevron title={t("settings.tabGeneral")} testID="settings-general" onPress={() => openPage("general")} />
      <ListRow chevron title={t("mobile.connection")} testID="settings-dns" onPress={() => openPage("dns")} />
      <ListRow chevron title={t("mobile.maintenance")} testID="settings-maintenance" onPress={() => openPage("maintenance")} />
      <ListRow last chevron title={t("mobile.about")} testID="settings-about" onPress={() => openPage("about")} />
    </ListGroup>
  </DetailScreen>;
}
