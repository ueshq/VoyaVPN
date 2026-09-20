import { render } from "@testing-library/react-native";

import { localeReady } from "~/native/platform-boot";

import { App } from "./App";
import { SHELL_TABS, type ShellTab } from "./tabs";

describe("App", () => {
  it("mounts a tab for every section, labelled from the shared locale", async () => {
    // Settling the startup locale first keeps the shell from suspending mid
    // render; what is under test is the wiring, not Suspense.
    await localeReady;

    // `render` is asynchronous in @testing-library/react-native v14 — awaiting
    // it is what puts the committed tree behind these queries.
    //
    // The whole shared chain has to resolve for this to pass: the i18n host on
    // MMKV, the preferences store behind `@voya/client`, and the translations
    // the tab labels come from.
    const view = await render(<App />);

    for (const label of ["Home", "Nodes", "Rules", "Network activity", "Settings"]) {
      expect(view.getAllByText(label).length).toBeGreaterThan(0);
    }

    expect(Object.keys(SHELL_TABS) as ShellTab[]).toHaveLength(5);
  });
});
