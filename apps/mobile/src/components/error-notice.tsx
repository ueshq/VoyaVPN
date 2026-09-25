import { IpcCommandError } from "@voya/client/errors";
import type { AppErrorKind } from "@voya/contracts";
import { Button } from "heroui-native/button";
import { Typography } from "heroui-native/text";
import { View } from "react-native";
import { useI18n } from "@voya/i18n/use-i18n";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import { Banner } from "./banner";
import { Disclosure } from "./disclosure";

/**
 * A failure in words the user can act on, with the redacted diagnostic behind
 * a `Disclosure`. Collapsed content is not mounted, so the raw message is
 * never on screen, nor in the accessibility tree, until asked for.
 */
export function ErrorNotice({ error, message, reason, retry, retryLabel }: { error: unknown; message?: string; reason?: AppErrorKind["type"]; retry?: () => void; retryLabel?: string }) {
  const { t } = useI18n();
  if (!error) return null;
  const kind = reason ?? (error instanceof IpcCommandError ? error.appError.kind.type : undefined);
  const reasons = {
    validation: t("mobile.invalidFields"), notFound: t("mobile.missingItem"), elevationRequired: t("home.authorizationDeclined"),
    unsupported: t("mobile.unsupportedAction"), missingCore: t("mobile.unsupportedAction"), network: t("mobile.networkFailed"),
    io: t("mobile.failed"), database: t("mobile.failed"), internal: t("mobile.failed"),
  } satisfies Record<AppErrorKind["type"], string>;
  return <View className="gap-2">
    <Banner status="danger" liveRegion message={message ?? (kind ? reasons[kind] : t("mobile.failed"))} />
    {retry ? <Button variant="secondary" className="min-h-12 h-auto self-start" onPress={retry}><Button.Label>{retryLabel ?? t("actions.retry")}</Button.Label></Button> : null}
    <Disclosure title={t("mobile.details")}>
      <Typography selectable className="text-sm text-muted">{redactOperationalError(error)}</Typography>
    </Disclosure>
  </View>;
}
