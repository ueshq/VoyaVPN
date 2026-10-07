import { act, cleanup, render, userEvent, waitFor } from "@testing-library/react-native";
import { createAppQueryClient } from "@voya/client/query-client";
import { usePreferencesStore } from "@voya/client/preferences-store";
import { IpcCommandError } from "@voya/client/errors";
import { Alert, Linking } from "react-native";

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
import { navigateToTab } from "./navigation";

// Keep the real cache and error handling, but retain the app-owned client so
// each test can release it after all query observers have unmounted.
jest.mock("@voya/client/query-client", () => {
  const actual = jest.requireActual("@voya/client/query-client");
  return { ...actual, createAppQueryClient: jest.fn(actual.createAppQueryClient) };
});

afterEach(async () => {
  await cleanup();
  for (const result of jest.mocked(createAppQueryClient).mock.results) {
    if (result.type === "return") result.value.clear();
  }
});

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
    const labels = { home: "Home", profiles: "Nodes", rules: "Rules", settings: "Settings" } satisfies Record<
      ShellTab,
      string
    >;

    // The bar derives both the visible label and the VoiceOver name from the
    // screen's title; nothing sets them separately.
    for (const label of Object.values(labels)) {
      expect(view.getByRole("tab", { name: label })).toHaveProp("accessibilityLabel", label);
      expect(view.getAllByText(label).length).toBeGreaterThan(0);
    }
  });

  it("marks the current tab selected through the native selection event", async () => {
    await localeReady;
    const view = await render(<App />);

    expect(view.getByRole("tab", { name: "Home" })).toBeSelected();
    expect(view.getByRole("tab", { name: "Settings" })).not.toBeSelected();

    await userEvent.setup().press(view.getByRole("tab", { name: "Settings" }));

    expect(view.getByRole("tab", { name: "Settings" })).toBeSelected();
    expect(view.getByRole("tab", { name: "Home" })).not.toBeSelected();
    // The tab keeps its full title for VoiceOver even where the label is short.
    expect(view.queryByRole("tab", { name: "Network activity" })).toBeNull();
  });

  it("updates native selection on a programmatic tab jump", async () => {
    await localeReady;
    const view = await render(<App />);

    await act(() => navigateToTab("rules"));
    expect(view.getByRole("tab", { name: "Rules" })).toBeSelected();
    expect(view.getByRole("tab", { name: "Home" })).not.toBeSelected();
  });

  it("reaches the subscription list from Settings", async () => {
    await localeReady;
    const view = await render(<App />);
    const user = userEvent.setup();

    await user.press(view.getByRole("tab", { name: "Settings" }));
    await user.press(view.getByTestId("settings-subscriptions"));

    expect(await view.findByText("Update all subscriptions")).toBeOnTheScreen();
  });

  it("labels the pushed page's back button for VoiceOver and goes back on tap", async () => {
    await localeReady;
    const view = await render(<App />);
    const user = userEvent.setup();

    await user.press(view.getByRole("tab", { name: "Settings" }));
    await user.press(view.getByTestId("settings-subscriptions"));
    await view.findByText("Update all subscriptions");

    // The chevron is drawn by the shell, not the native bar, so it carries an
    // accessibility label the native UIKit button could not be given.
    await user.press(await view.findByLabelText("Back"));

    await waitFor(() => expect(view.queryByText("Update all subscriptions")).toBeNull());
    expect(view.getByRole("tab", { name: "Home" })).toBeOnTheScreen();
    expect(view.getByRole("tab", { name: "Settings" })).toBeSelected();
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
    expect(view.queryByRole("tab", { name: "Home" })).toBeNull();

    await userEvent.setup().press(view.getByTestId("privacy-continue"));

    expect(view.getByRole("tab", { name: "Home" })).toBeOnTheScreen();
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
    // The address rides in the link's accessibility label instead of being
    // printed again below it.
    expect(view.getByTestId("privacy-policy-link")).toHaveProp(
      "accessibilityLabel",
      `Privacy policy: ${PRIVACY_POLICY_URL}`,
    );
    openURL.mockRestore();
  });

  it("takes over with the reset flow when the database is rejected", async () => {
    await localeReady;
    const backend = mockTransport();
    const rejectedDatabase = {
      kind: { type: "database", code: "schemaUnsupported", resetCommand: "rm -f x" },
      message: "unsupported Voya database schema",
      subsystem: "app",
    } as const;
    backend.commands.loadAppSettings = async () => {
      throw new IpcCommandError(rejectedDatabase);
    };
    registerMobileBackend(backend);

    const view = await render(<App />);
    const user = userEvent.setup();

    // Nothing else can run: navigation is replaced, not covered.
    expect(view.queryByRole("tab", { name: "Home" })).toBeNull();
    expect(view.getByText("VoyaVPN could not start")).toBeOnTheScreen();

    // Two steps before the destructive action.
    await user.press(view.getByTestId("startup-reset"));
    expect(view.getByText("Reset the database?")).toBeOnTheScreen();
    await user.press(view.getByTestId("startup-reset-cancel"));
    expect(view.queryByText("Reset the database?")).toBeNull();
  });

  it("comes back up on the fresh database once the reset is confirmed", async () => {
    await localeReady;
    const backend = mockTransport();
    const loadAppSettings = backend.commands.loadAppSettings;
    let rejected = true;
    let loads = 0;
    backend.commands.loadAppSettings = async () => {
      loads += 1;
      if (rejected) {
        throw new IpcCommandError({
          kind: { type: "database", code: "corrupt", resetCommand: "rm -f x" },
          message: "the database is not a database",
          subsystem: "app",
        });
      }
      return loadAppSettings();
    };
    // The host moves the database aside; the next command opens a new one.
    backend.resetApplicationData = async () => {
      rejected = false;
    };
    registerMobileBackend(backend);
    // A returning user, so what follows the reset is the app itself.
    usePreferencesStore.setState({ privacyNoticeVersion: PRIVACY_NOTICE_VERSION });

    const view = await render(<App />);
    const user = userEvent.setup();
    await user.press(view.getByTestId("startup-reset"));
    const loadsBeforeReset = loads;
    await user.press(view.getByTestId("startup-reset-confirm"));

    await waitFor(() => expect(view.getByRole("tab", { name: "Home" })).toBeOnTheScreen());
    // One read for the retry: the shell reads the answer the gate already has.
    expect(loads).toBe(loadsBeforeReset + 1);
  });

  it("says so when the reset itself fails, and lets it be tried again", async () => {
    await localeReady;
    const backend = mockTransport();
    backend.commands.loadAppSettings = async () => {
      throw new IpcCommandError({
        kind: { type: "database", code: "corrupt", resetCommand: "rm -f x" },
        message: "the database is not a database",
        subsystem: "app",
      });
    };
    backend.resetApplicationData = async () => {
      throw new Error("the database could not be moved aside");
    };
    registerMobileBackend(backend);

    const view = await render(<App />);
    const user = userEvent.setup();
    await user.press(view.getByTestId("startup-reset"));
    await user.press(view.getByTestId("startup-reset-confirm"));

    // A button that merely re-enabled would read as a reset that did nothing.
    expect(await view.findByTestId("banner-danger")).toBeOnTheScreen();
    expect(view.getByTestId("startup-reset-confirm")).toBeEnabled();
  });

  it("returns to the subscription list once a subscription is deleted", async () => {
    await localeReady;
    registerMobileBackend(mockTransport());
    usePreferencesStore.setState({ privacyNoticeVersion: PRIVACY_NOTICE_VERSION });
    // The real navigator, because the page guards its own removal while it is
    // busy, and the editor unmounts the moment its subscription is gone: the
    // way back has to survive both.
    const alert = jest.spyOn(Alert, "alert").mockImplementation(() => {});
    const view = await render(<App />);
    const user = userEvent.setup();

    await user.press(view.getByRole("tab", { name: "Settings" }));
    await user.press(view.getByTestId("settings-subscriptions"));
    await user.press(await view.findByText("Example provider"));
    await user.press(await view.findByText("Delete"));
    const buttons = alert.mock.calls.at(-1)?.[2] ?? [];
    await act(async () => buttons.find((button) => button.style === "destructive")?.onPress?.());

    await waitFor(() => expect(view.getByText("No subscriptions")).toBeOnTheScreen());
    expect(view.queryByText("Subscription details")).toBeNull();
    alert.mockRestore();
  });

  it("asks again once the notice has a newer version than the one accepted", () => {
    expect(isPrivacyNoticeAccepted(null)).toBe(false);
    expect(isPrivacyNoticeAccepted(PRIVACY_NOTICE_VERSION - 1)).toBe(false);
    expect(isPrivacyNoticeAccepted(PRIVACY_NOTICE_VERSION)).toBe(true);
  });
});
