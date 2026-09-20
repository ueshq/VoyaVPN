import { i18next } from "@voya/i18n/core";

import { SHELL_TABS, type ShellTab } from "./tabs";

describe("SHELL_TABS", () => {
  it("names the five phone sections", () => {
    // `selfHost` is deliberately absent: a phone is not an exit node.
    expect(Object.keys(SHELL_TABS)).toEqual(["home", "profiles", "rules", "connections", "settings"]);
  });

  it("labels every tab with a key the shipped locale defines", () => {
    // `TranslationKey` proves the key exists in en.json at compile time; this
    // proves the running i18next instance resolves it, which is what the tab
    // bar actually calls.
    for (const tab of Object.keys(SHELL_TABS) as ShellTab[]) {
      expect(i18next.exists(SHELL_TABS[tab].titleKey)).toBe(true);
    }
  });
});
