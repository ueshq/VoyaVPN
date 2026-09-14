import type { SubscriptionUpdateResult } from "@/ipc/bindings";
import type { TranslationFunction } from "@voya/i18n";
import { redactOperationalMessage } from "@voya/utils/operational-redaction";

/**
 * A subscription update that failed to download is *not* reported as an error:
 * the backend folds the fetch failure into `skipped` + `messages` and still
 * returns `Ok` so a batch update is never aborted by one dead source
 * (`prepare_subscription_snapshot`). Nothing imported, nothing updated, and a
 * skip or a per-source message to show for it is therefore the frontend's
 * failure signal — the same condition the auto-update scheduler treats as a
 * failed run.
 */
export function isSubscriptionUpdateFailure(result: SubscriptionUpdateResult) {
  return (
    result.imported === 0
    && result.updated === 0
    && (result.skipped > 0 || result.messages.length > 0)
  );
}

/**
 * Throws the per-source reasons when an update brought nothing in (see
 * {@link isSubscriptionUpdateFailure}); a failure without a reason says so.
 */
export function assertSubscriptionUpdated(
  result: SubscriptionUpdateResult,
  t: TranslationFunction,
) {
  if (isSubscriptionUpdateFailure(result)) {
    throw new Error(
      subscriptionUpdateMessages(result) || t("panes.subscriptions.updateNothingImported"),
    );
  }
}

/**
 * The per-source reasons, one per line, ready for display. They are backend
 * `error.to_string()` values that can embed the subscription URL (and with it a
 * token), so every line goes through the shared redaction helper first.
 */
export function subscriptionUpdateMessages(result: SubscriptionUpdateResult) {
  return result.messages.map((message) => redactOperationalMessage(message)).join("\n");
}

/**
 * The banner line for a finished update. An update deletes the nodes a source
 * stopped offering, which can include the one in use, so that count is named
 * rather than folded into "updated".
 */
export function formatSubscriptionUpdateSummary(
  result: SubscriptionUpdateResult,
  t: TranslationFunction,
) {
  const counts = { imported: result.imported, updated: result.updated };
  return result.removedExisting > 0
    ? t("panes.subscriptions.updateResultWithRemoved", { ...counts, removed: result.removedExisting })
    : t("panes.subscriptions.updateResult", counts);
}
