import { beforeEach, describe, expect, it } from "vitest";

import { usePreferencesStore } from "./preferences-store";
import { installTestStorage } from "./test-storage";

const STORAGE_KEY = "voyavpn.preferences";
const storage = installTestStorage();

describe("preferences store", () => {
  beforeEach(() => {
    storage.clear();
    usePreferencesStore.setState({ ruleLibraryUpdatedAt: null, themeMode: "system", themePreview: null });
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
});
