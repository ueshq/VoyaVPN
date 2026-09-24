import { beforeEach, describe, expect, it } from "vitest";
import { changeLocale, i18next } from "@voya/i18n";
import type { SubscriptionUpdateOutcome, SubscriptionUpdateResult } from "@voya/contracts";
import { assertSubscriptionUpdated, formatSubscriptionUpdateSummary, isSubscriptionUpdateFailure, subscriptionUpdateMessages } from "./subscription-update-result";

const success: SubscriptionUpdateOutcome = { subscriptionId: "a", status: "success", reason: "updated", imported: 2, removedExisting: 0, diagnostic: null };
const failed: SubscriptionUpdateOutcome = { ...success, subscriptionId: "b", status: "failed", reason: "downloadFailed", imported: 0 };
function result(outcomes: SubscriptionUpdateOutcome[] = [], patch: Partial<SubscriptionUpdateResult> = {}): SubscriptionUpdateResult {
  return { imported: 0, messages: [], removedExisting: 0, skipped: 0, updated: 0, outcomes, ...patch };
}
beforeEach(async () => { await changeLocale("en", { persist: false }); });
describe("subscription results", () => {
  it("uses typed outcomes for all-success, partial-success, all-failure and skipped batches", () => {
    expect(isSubscriptionUpdateFailure(result([success], { messages: ["imported 2 nodes"] }))).toBe(false);
    expect(isSubscriptionUpdateFailure(result([success, failed]))).toBe(false);
    expect(isSubscriptionUpdateFailure(result([failed]))).toBe(true);
    expect(isSubscriptionUpdateFailure(result([{ ...failed, status: "skipped", reason: "sourceChanged" }]))).toBe(false);
    expect(isSubscriptionUpdateFailure(result())).toBe(false);
  });
  it("success diagnostics never become error banners", () => {
    expect(subscriptionUpdateMessages(result([success], { messages: ["imported 2 nodes"] }), i18next.t.bind(i18next))).toBe("");
  });
  it("localizes failures without showing network diagnostics", () => {
    expect(() => assertSubscriptionUpdated(result([failed]), i18next.t.bind(i18next))).toThrow("Subscription update failed");
    expect(subscriptionUpdateMessages(result([{ ...failed, diagnostic: "https://user:secret@example.test/sub?token=private" }]))).not.toMatch(/secret|private/);
  });
  it("explains skipped updates separately from failures", () => {
    expect(subscriptionUpdateMessages(result([{ ...failed, status: "skipped", reason: "sourceChanged" }]), i18next.t.bind(i18next))).toContain("changed while downloading");
  });
  it("names removed nodes in the summary", () => {
    expect(formatSubscriptionUpdateSummary(result([success], { imported: 5, updated: 1 }), i18next.t.bind(i18next))).toBe("1 updated, 5 nodes imported");
    expect(formatSubscriptionUpdateSummary(result([success], { imported: 5, updated: 1, removedExisting: 2 }), i18next.t.bind(i18next))).toBe("1 updated, 5 nodes imported, 2 nodes no longer offered were removed");
  });
});
