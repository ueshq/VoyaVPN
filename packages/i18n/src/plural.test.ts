import { afterEach, describe, expect, it } from "vitest";

import { changeLocale, i18next } from "./index";

describe("plural messages", () => {
  afterEach(async () => {
    await changeLocale("en", { persist: false });
  });

  it("picks the form from a numeric count", () => {
    expect(i18next.t("nodeGroups.membersCount", { count: 1 })).toBe("1 node");
    expect(i18next.t("nodeGroups.membersCount", { count: 2 })).toBe("2 nodes");
    expect(i18next.t("nodeGroups.membersCount", { count: 0 })).toBe("0 nodes");
  });

  it("formats a large count in the app's language", () => {
    expect(i18next.t("panes.profiles.import.summary.imported", { count: 1 })).toBe(
      "Imported 1 node.",
    );
    expect(i18next.t("panes.profiles.import.summary.imported", { count: 12_345 })).toBe(
      "Imported 12,345 nodes.",
    );
  });

  it("reads the same in Chinese whatever the count", async () => {
    await changeLocale("zh-Hans", { persist: false });

    expect(i18next.t("nodeGroups.membersCount", { count: 1 })).toBe(
      i18next.t("nodeGroups.membersCount", { count: 5 }).replace("5", "1"),
    );
  });
});
