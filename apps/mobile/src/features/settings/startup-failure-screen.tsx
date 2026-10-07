import { Button } from "heroui-native/button";
import { Typography } from "heroui-native/text";
import { useState } from "react";
import { useI18n } from "@voya/i18n/use-i18n";
import type { AppError } from "@voya/contracts";
import { redactOperationalMessage } from "@voya/utils/operational-redaction";

import { DetailScreen } from "~/components/detail-screen";
import { ErrorNotice } from "~/components/error-notice";
import { Disclosure } from "~/components/disclosure";
import { PageHeader } from "~/components/page-header";

/**
 * The takeover a rejected database shows.
 *
 * Nothing else can run while the backend cannot open its database, so this
 * replaces navigation entirely. The words are the desktop's `startupFailure`
 * strings: one incident, one explanation, on both platforms. The two-step
 * confirmation is deliberate — the reset is the only irreversible action in
 * the app.
 */
export function StartupFailureScreen({
  error,
  busy,
  resetFailure = null,
  onReset,
}: {
  error: AppError;
  busy: boolean;
  /** Why the last reset did not happen, when one was tried and failed. */
  resetFailure?: unknown;
  onReset: () => void;
}) {
  const { t } = useI18n();
  const [confirming, setConfirming] = useState(false);

  return (
    <DetailScreen accessibilityLabel={t("startupFailure.title")}>
      <PageHeader title={t("startupFailure.title")} />
      {/* The confirm texts say the same thing concretely, so the longer
          explanation and the disclosure step aside for them — keeping the
          actions above the fold on the smallest window. */}
      {confirming ? null : (
        <>
          <Typography className="text-base text-foreground">{t("startupFailure.resetExplanation")}</Typography>
          <Disclosure title={t("mobile.details")}>
            <Typography className="text-sm text-subtle" selectable>
              {redactOperationalMessage(error.message)}
            </Typography>
          </Disclosure>
        </>
      )}
      {confirming ? (
        <>
          <Typography className="text-base font-semibold text-foreground">
            {t("startupFailure.confirmTitle")}
          </Typography>
          <Typography className="text-base text-foreground">{t("startupFailure.confirmMessage")}</Typography>
          <ErrorNotice error={resetFailure} />
          <Button testID="startup-reset-confirm" isDisabled={busy} onPress={onReset}>
            <Button.Label>{t("startupFailure.confirmReset")}</Button.Label>
          </Button>
          <Button
            testID="startup-reset-cancel"
            variant="secondary"
            isDisabled={busy}
            onPress={() => setConfirming(false)}
          >
            <Button.Label>{t("actions.cancel")}</Button.Label>
          </Button>
        </>
      ) : (
        <Button testID="startup-reset" onPress={() => setConfirming(true)}>
          <Button.Label>{t("startupFailure.resetDatabase")}</Button.Label>
        </Button>
      )}
    </DetailScreen>
  );
}
