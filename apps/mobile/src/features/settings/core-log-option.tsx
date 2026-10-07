import { useAppSettings } from "@voya/features/settings/use-app-settings";
import { useI18n } from "@voya/i18n/use-i18n";
import { ListGroup } from "heroui-native/list-group";
import { SwitchRow } from "~/components/switch-row";
import { ErrorNotice } from "~/components/error-notice";

export function CoreLogOption() {
  const { t } = useI18n();
  const app = useAppSettings();
  return <>
    <ErrorNotice error={app.error} retry={app.retry} />
    {app.settings ? <ListGroup><SwitchRow last label={t("settings.core.logEnabled")} value={app.settings.core.logEnabled} onChange={(logEnabled) => app.update((current) => ({ ...current, core: { ...current.core, logEnabled } }))} /></ListGroup> : null}
  </>;
}
