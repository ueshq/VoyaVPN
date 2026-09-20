import { beforeEach, describe, expect, it } from "vitest";

import { changeLocale, i18next } from "@voya/i18n";

import type { SubscriptionUpdateResult } from "@voya/contracts";

import {
  assertSubscriptionUpdated,
  formatSubscriptionUpdateSummary,
  isSubscriptionUpdateFailure,
  subscriptionUpdateMessages,
} from "./subscription-update-result";

function result(overrides: Partial<SubscriptionUpdateResult> = {}): SubscriptionUpdateResult {
  return {
    imported: 0,
    messages: [],
    removedExisting: 0,
    skipped: 0,
    updated: 0,
    ...overrides,
  };
}

describe("isSubscriptionUpdateFailure", () => {
  it("treats a skipped source that imported nothing as a failure", () => {
    expect(isSubscriptionUpdateFailure(result({ skipped: 1 }))).toBe(true);
  });

  it("treats a reported message with no import as a failure", () => {
    expect(isSubscriptionUpdateFailure(result({ messages: ["Airport->request failed"] }))).toBe(
      true,
    );
  });

  it("accepts an update that imported or updated something", () => {
    expect(isSubscriptionUpdateFailure(result({ imported: 3 }))).toBe(false);
    expect(isSubscriptionUpdateFailure(result({ updated: 1 }))).toBe(false);
    // A partial batch failure still counts as a success overall.
    expect(
      isSubscriptionUpdateFailure(result({ messages: ["Backup->request failed"], skipped: 1, updated: 1 })),
    ).toBe(false);
  });

  it("accepts a no-op update that reported nothing", () => {
    expect(isSubscriptionUpdateFailure(result())).toBe(false);
  });
});

describe("assertSubscriptionUpdated", () => {
  beforeEach(async () => {
    await changeLocale("en", { persist: false });
  });

  it("accepts an update that brought something in", () => {
    const t = i18next.t.bind(i18next);

    expect(() => assertSubscriptionUpdated(result({ imported: 1, skipped: 1 }), t)).not.toThrow();
  });

  it("throws the redacted reasons, or that nothing was imported", () => {
    const t = i18next.t.bind(i18next);

    expect(() =>
      assertSubscriptionUpdated(
        result({ messages: ["Airport->request failed https://example.test/sub?token=private"] }),
        t,
      ),
    ).toThrow("Airport->request failed [redacted URL]");
    expect(() => assertSubscriptionUpdated(result({ skipped: 1 }), t)).toThrow(
      t("panes.subscriptions.updateNothingImported"),
    );
  });
});

describe("subscriptionUpdateMessages", () => {
  it("redacts the subscription URL out of each reason", () => {
    const messages = subscriptionUpdateMessages(
      result({
        messages: [
          "Airport->request failed https://user:secret@example.test/sub?token=private",
          "Backup->no importable nodes were found",
        ],
      }),
    );

    expect(messages).toContain("Airport->request failed");
    expect(messages).not.toContain("secret");
    expect(messages).not.toContain("private");
    expect(messages.split("\n")).toHaveLength(2);
  });

  it("returns an empty string when the backend reported no reason", () => {
    expect(subscriptionUpdateMessages(result({ skipped: 1 }))).toBe("");
  });
});

describe("formatSubscriptionUpdateSummary", () => {
  beforeEach(async () => {
    await changeLocale("en", { persist: false });
  });

  it("names the nodes a source stopped offering", () => {
    const t = i18next.t.bind(i18next);

    expect(formatSubscriptionUpdateSummary(result({ imported: 5, updated: 1 }), t)).toBe(
      "1 updated, 5 nodes imported",
    );
    expect(
      formatSubscriptionUpdateSummary(result({ imported: 5, removedExisting: 2, updated: 1 }), t),
    ).toBe("1 updated, 5 nodes imported, 2 nodes no longer offered were removed");
  });
});
