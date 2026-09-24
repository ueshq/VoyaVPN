import { IpcCommandError } from "@voya/client/errors";
import type { AppErrorKind } from "@voya/contracts";
import { useState } from "react";
import { Button } from "heroui-native/button";
import { Typography } from "heroui-native/text";
import { View } from "react-native";
import { useI18n } from "@voya/i18n/use-i18n";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import { Banner } from "./banner";

export function ErrorNotice({ error, message, reason, retry, retryLabel }: { error: unknown; message?: string; reason?: AppErrorKind["type"]; retry?: () => void; retryLabel?: string }) {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState(false);
  if (!error) return null;
  const kind = reason ?? (error instanceof IpcCommandError ? error.appError.kind.type : undefined);
  const reasons = {
    validation: t("mobile.invalidFields"), notFound: t("mobile.missingItem"), elevationRequired: t("home.authorizationDeclined"),
    unsupported: t("mobile.unsupportedAction"), missingCore: t("mobile.unsupportedAction"), network: t("mobile.networkFailed"),
    io: t("mobile.failed"), database: t("mobile.failed"), internal: t("mobile.failed"),
  } satisfies Record<AppErrorKind["type"], string>;
  return <View className="gap-2">
    <Banner status="danger" liveRegion message={message ?? (kind ? reasons[kind] : t("mobile.failed"))} />
    <View className="flex-row flex-wrap gap-2">
      {retry ? <Button variant="secondary" className="min-h-12 h-auto" onPress={retry}><Button.Label>{retryLabel ?? t("actions.retry")}</Button.Label></Button> : null}
      <Button variant="ghost" className="min-h-12 h-auto" onPress={() => setExpanded(!expanded)} accessibilityState={{ expanded }}><Button.Label>{t("mobile.details")}</Button.Label></Button>
    </View>
    {expanded ? <Typography selectable className="text-sm text-subtle">{redactOperationalError(error)}</Typography> : null}
  </View>;
}
