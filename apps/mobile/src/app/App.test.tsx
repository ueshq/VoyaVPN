import { render, userEvent } from "@testing-library/react-native";
import { usePreferencesStore } from "@voya/client/preferences-store";
import { Linking } from "react-native";

import {
  PRIVACY_NOTICE_VERSION,
  PRIVACY_POLICY_URL,
  SUPPORT_URL,
  isPrivacyNoticeAccepted,
} from "~/features/settings/privacy-notice";
import { registerMobileBackend } from "~/ipc/platform";
import { localeReady } from "~/native/platform-boot";
import { mockTransport } from "~/test/mock-transport";

import { App } from "./App";
import type { ShellTab } from "./tabs";

beforeEach(() => {
  registerMobileBackend(mockTransport());
  // A returning user: the first-run data notice was accepted on an earlier launch.
  usePreferencesStore.setState({ privacyNoticeVersion: PRIVACY_NOTICE_VERSION });
});

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

    // One entry per tab, checked by the type; `selfHost` is deliberately
    // absent, because a phone is not an exit node.
    const labels = { home: "Home", profiles: "Nodes", rules: "Rules", settings: "Settings" } satisfies Record<ShellTab, string>;

    // The bar derives both the visible label and the VoiceOver name from the
    // screen's title; nothing sets them separately.
    for (const [tab, label] of Object.entries(labels)) {
      expect(view.getByTestId(`tab-${tab}`)).toHaveProp("accessibilityLabel", label);
      expect(view.getAllByText(label).length).toBeGreaterThan(0);
    }
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

describe("first-run data notice", () => {
  beforeEach(() => usePreferencesStore.setState({ privacyNoticeVersion: null }));

  it("stands in for the whole app until it is accepted, then stays accepted", async () => {
    await localeReady;
    const view = await render(<App />);

    expect(view.getByText("Before you start")).toBeOnTheScreen();
    expect(view.getByText(/It collects no data/)).toBeOnTheScreen();
    expect(view.getByText(/raw\.githubusercontent\.com/)).toBeOnTheScreen();
    // Nothing of the app is reachable behind it.
    expect(view.queryByTestId("tab-home")).toBeNull();

    await userEvent.setup().press(view.getByTestId("privacy-continue"));

    expect(view.getByTestId("tab-home")).toBeOnTheScreen();
    expect(view.queryByText("Before you start")).toBeNull();
    expect(usePreferencesStore.getState().privacyNoticeVersion).toBe(PRIVACY_NOTICE_VERSION);
  });

  it("opens the privacy policy and support pages", async () => {
    await localeReady;
    const openURL = jest.spyOn(Linking, "openURL").mockResolvedValue(undefined);
    const view = await render(<App />);
    const user = userEvent.setup();

    await user.press(view.getByTestId("privacy-policy-link"));
    await user.press(view.getByTestId("support-link"));

    expect(openURL.mock.calls).toEqual([[PRIVACY_POLICY_URL], [SUPPORT_URL]]);
    // The addresses are also on screen, to read or copy.
    expect(view.getByText(PRIVACY_POLICY_URL)).toBeOnTheScreen();
    openURL.mockRestore();
  });

  it("asks again once the notice has a newer version than the one accepted", () => {
    expect(isPrivacyNoticeAccepted(null)).toBe(false);
    expect(isPrivacyNoticeAccepted(PRIVACY_NOTICE_VERSION - 1)).toBe(false);
    expect(isPrivacyNoticeAccepted(PRIVACY_NOTICE_VERSION)).toBe(true);
  });
});
