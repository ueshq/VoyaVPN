import { useHomeRuntime } from "@voya/features/home/use-home-runtime";
import { exitIpLabel } from "@voya/features/home/exit-ip-label";
import { useI18n } from "@voya/i18n/use-i18n";
import { formatDurationMs } from "@voya/utils/formatting";
import { Button } from "heroui-native/button";
import { ListGroup } from "heroui-native/list-group";
import { Typography } from "heroui-native/text";
import { DetailScreen } from "~/components/detail-screen";
import { ListRow } from "~/components/list-row";
import { openPage } from "~/app/navigation";
import { WorldMap } from "./world-map";
import { useConnectedDurationMs } from "./use-connected-duration";

export function SessionDetailsScreen() {
  const { t } = useI18n();
  const runtime = useHomeRuntime();
  const duration = useConnectedDurationMs();
  return <DetailScreen>
    {runtime.connected ? <>
      <WorldMap marker={runtime.marker} />
      <ListGroup>
        <ListRow title={t("home.duration")} description={formatDurationMs(duration ?? 0)} />
        <ListRow last title={t("home.exitIp")} description={exitIpLabel(runtime.exitIp, t)} />
      </ListGroup>
      <Button variant="secondary" onPress={() => openPage("activity")}><Button.Label>{t("tabs.connections")}</Button.Label></Button>
    </> : <Typography className="text-base text-subtle">{t("status.disconnected")}</Typography>}
    <Button variant="secondary" onPress={() => openPage("logs")}><Button.Label>{t("mobile.diagnostics")}</Button.Label></Button>
  </DetailScreen>;
}
