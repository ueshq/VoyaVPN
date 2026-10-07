import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { useI18n } from "@voya/i18n/use-i18n";
import { formatDateTime } from "@voya/utils/format-date-time";
import { Button } from "heroui-native/button";
import { Typography } from "heroui-native/text";
import { useState } from "react";
import type { RootRoutes } from "~/app/navigation";
import { DetailScreen } from "~/components/detail-screen";
import { ErrorNotice } from "~/components/error-notice";
import { useBusyAction } from "@voya/features/forms/use-busy-action";
import { connectionBytes } from "@voya/features/proxy/connection-display";
import { closeSingleConnection } from "./connection-actions";

export function ConnectionDetailsScreen({
  route,
  navigation,
}: NativeStackScreenProps<RootRoutes, "connectionDetails">) {
  const { connection } = route.params;
  const { language, t } = useI18n();
  const [error, setError] = useState<unknown>(null);
  const { busy, run } = useBusyAction();
  function close() {
    const { id } = connection;
    if (!id) return;
    void run(async () => {
      try {
        await closeSingleConnection(id);
        navigation.goBack();
      } catch (failure) {
        setError(failure);
      }
    });
  }
  return (
    <DetailScreen>
      <Typography selectable className="text-xl font-semibold text-foreground">
        {connection.host}
      </Typography>
      <Typography selectable className="text-base text-foreground">
        {[connection.source, connection.destination].join(" → ")}
      </Typography>
      <Typography className="text-base text-subtle">
        {[
          connection.network,
          connection.connectionType,
          connection.start ? formatDateTime(connection.start, language) : null,
        ]
          .filter(Boolean)
          .join(" · ")}
      </Typography>
      <Typography className="text-base text-foreground">
        {t("mobile.upload")} {connectionBytes(connection.upload)} · {t("mobile.download")}{" "}
        {connectionBytes(connection.download)}
      </Typography>
      <Typography selectable className="text-base text-foreground">
        {connection.chains.join(" → ")}
      </Typography>
      {connection.rule ? (
        <Typography selectable className="text-base text-subtle">
          {connection.rule} {connection.rulePayload}
        </Typography>
      ) : null}
      {connection.process ? (
        <Typography selectable className="text-base text-subtle">
          {connection.process} {connection.processPath}
        </Typography>
      ) : null}
      <ErrorNotice error={error} />
      <Button variant="danger" isDisabled={!connection.id || busy} onPress={close}>
        <Button.Label>{t("actions.disconnect")}</Button.Label>
      </Button>
    </DetailScreen>
  );
}
