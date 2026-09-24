import type { SubscriptionUpdateResult } from "@voya/contracts";
import type { TranslationFunction, TranslationKey } from "@voya/i18n/core";
import { redactOperationalMessage } from "@voya/utils/operational-redaction";

/** Outcome state is authoritative; diagnostic messages can include successes. */
export function isSubscriptionUpdateFailure(result: SubscriptionUpdateResult) {
  return result.outcomes.some((item) => item.status === "failed") && !result.outcomes.some((item) => item.status === "success");
}

const REASONS = {
  downloadFailed: "mobile.updateFailed",
  emptyContent: "mobile.updateEmpty",
  invalidFilter: "mobile.updateFilter",
  invalidSource: "mobile.updateFailed",
  noImportableNodes: "mobile.updateEmpty",
  sourceChanged: "mobile.updateChanged",
  updated: "mobile.saved",
} satisfies Record<SubscriptionUpdateResult["outcomes"][number]["reason"], TranslationKey>;

export function assertSubscriptionUpdated(result: SubscriptionUpdateResult, t: TranslationFunction) {
  if (isSubscriptionUpdateFailure(result)) {
    throw new Error(subscriptionUpdateMessages(result, t) || t("panes.subscriptions.updateNothingImported"));
  }
}

export function subscriptionUpdateMessages(result: SubscriptionUpdateResult, t?: TranslationFunction) {
  return result.outcomes.filter((item) => item.status !== "success").map((item) =>
    t ? t(REASONS[item.reason]) : redactOperationalMessage(item.diagnostic ?? item.reason),
  ).join("\n");
}

export function formatSubscriptionUpdateSummary(result: SubscriptionUpdateResult, t: TranslationFunction) {
  const counts = { imported: result.imported, updated: result.updated };
  return result.removedExisting > 0
    ? t("panes.subscriptions.updateResultWithRemoved", { ...counts, removed: result.removedExisting })
    : t("panes.subscriptions.updateResult", counts);
}
