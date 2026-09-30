import { beforeEach, describe, expect, it } from "vitest";

import { usePreferencesStore } from "./preferences-store";
import { installTestStorage } from "./test-storage";

const STORAGE_KEY = "voyavpn.preferences";
const storage = installTestStorage();

describe("preferences store", () => {
  beforeEach(() => {
    storage.clear();
    usePreferencesStore.setState({
      privacyNoticeVersion: null,
      ruleLibraryUpdatedAt: null,
      themeMode: "system",
      themePreview: null,
    });
  });

  it("remembers when the rule library was last updated", () => {
    usePreferencesStore.getState().setRuleLibraryUpdatedAt(1_700_000_000_000);

    const stored = JSON.parse(storage.read(STORAGE_KEY) ?? "{}") as { state?: unknown };
    expect(stored.state).toMatchObject({ ruleLibraryUpdatedAt: 1_700_000_000_000, themeMode: "system" });
  });

  it("ignores a stored update time that is not a time", async () => {
    storage.write(
      STORAGE_KEY,
      JSON.stringify({ state: { ruleLibraryUpdatedAt: "yesterday", themeMode: "dark" }, version: 0 }),
    );

    await usePreferencesStore.persist.rehydrate();

    expect(usePreferencesStore.getState()).toMatchObject({ ruleLibraryUpdatedAt: null, themeMode: "dark" });
  });

  it("remembers which data notice the user accepted", () => {
    usePreferencesStore.getState().acceptPrivacyNotice(1);

    const stored = JSON.parse(storage.read(STORAGE_KEY) ?? "{}") as { state?: unknown };
    expect(stored.state).toMatchObject({ privacyNoticeVersion: 1 });
  });

  it("asks again when the stored acceptance is missing or not a version", async () => {
    // A build from before the notice wrote no such field.
    storage.write(STORAGE_KEY, JSON.stringify({ state: { themeMode: "dark" }, version: 0 }));
    await usePreferencesStore.persist.rehydrate();
    expect(usePreferencesStore.getState().privacyNoticeVersion).toBeNull();

    for (const privacyNoticeVersion of ["yes", 0, 1.5, true]) {
      storage.write(STORAGE_KEY, JSON.stringify({ state: { privacyNoticeVersion }, version: 0 }));
      await usePreferencesStore.persist.rehydrate();
      expect(usePreferencesStore.getState().privacyNoticeVersion).toBeNull();
    }
  });
});
