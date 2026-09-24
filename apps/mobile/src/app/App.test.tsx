import { render, userEvent } from "@testing-library/react-native";

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

    for (const label of ["Home", "Nodes", "Rules", "Settings"]) {
      expect(view.getAllByText(label).length).toBeGreaterThan(0);
    }

    expect(Object.keys(SHELL_TABS) as ShellTab[]).toHaveLength(4);
  });

  it("marks the current tab selected on the floating bar, the state XCUITest reads", async () => {
    await localeReady;
    const view = await render(<App />);

    expect(view.getByTestId("tab-home")).toBeSelected();
    expect(view.getByTestId("tab-settings")).not.toBeSelected();

    await userEvent.setup().press(view.getByTestId("tab-settings"));

    expect(view.getByTestId("tab-settings")).toBeSelected();
    expect(view.getByTestId("tab-home")).not.toBeSelected();
    // The tab keeps its full title for VoiceOver even where the label is short.
    expect(view.queryByTestId("tab-connections")).toBeNull();
  });

  it("reaches the subscription list from Settings", async () => {
    await localeReady;
    const view = await render(<App />);
    const user = userEvent.setup();

    await user.press(view.getByTestId("tab-settings"));
    await user.press(view.getByTestId("settings-subscriptions"));

    expect(await view.findByText("Update all subscriptions")).toBeOnTheScreen();
  });
});
