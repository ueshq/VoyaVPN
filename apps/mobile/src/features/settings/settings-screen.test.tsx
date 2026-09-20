import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, userEvent, waitFor } from "@testing-library/react-native";
import type { MockBackend } from "@voya/client/mock-backend";
import { usePreferencesStore } from "@voya/client/preferences-store";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { i18nHost } from "@voya/i18n/core";
import type { ReactNode } from "react";

import { registerMobileBackend, voyaTransport } from "~/ipc/platform";
import { localeReady } from "~/native/platform-boot";

import { SettingsScreen } from "./settings-screen";

let activeQueryClient: QueryClient | null = null;

async function renderSettings() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  activeQueryClient = queryClient;
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  return { queryClient, ...(await render(<SettingsScreen />, { wrapper })) };
}

function backend() {
  return voyaTransport() as MockBackend;
}

beforeAll(async () => {
  await localeReady;
});

beforeEach(() => {
  registerMobileBackend();
  useRuntimeEventStore.setState(useRuntimeEventStore.getInitialState());
});

afterEach(async () => {
  activeQueryClient?.clear();
  activeQueryClient = null;
  // The locale is process-wide, and a test that switches it would otherwise
  // hand the next one a Chinese UI.
  await i18nHost().changeLocale("en", { persist: false });
  usePreferencesStore.setState(usePreferencesStore.getInitialState());
});

beforeEach(() => {
  // The timestamp is persisted, so a run that updated it would leak into the
  // next test's "never updated" line.
  usePreferencesStore.setState({ ruleLibraryUpdatedAt: null });
});

describe("SettingsScreen", () => {
  it("marks the saved theme and saves another one", async () => {
    await renderSettings();

    expect(await screen.findByRole("button", { name: "Follow system" })).toBeSelected();

    await userEvent.setup().press(screen.getByRole("button", { name: "Dark" }));

    await waitFor(() => expect(backend().state.settings.appearance.theme).toBe("dark"));
    // Previewing is the point of the write: the store carries the theme before
    // the backend has answered, and the whole app follows it.
    expect(usePreferencesStore.getState().themeMode).toBe("dark");
  });

  it("switches the interface language, in that language", async () => {
    await renderSettings();

    await userEvent.setup().press(await screen.findByText("简体中文"));

    expect(await screen.findByText("语言")).toBeOnTheScreen();
    expect(screen.getByText("主题")).toBeOnTheScreen();
    await waitFor(() => expect(backend().state.settings.appearance.language).toBe("zh-Hans"));
  });

  it("saves a behaviour switch through the backend", async () => {
    await renderSettings();

    const toggle = await screen.findByRole("switch", {
      name: "Check the exit IP after connecting",
    });
    await fireEvent(toggle, "valueChange", false);

    await waitFor(() => expect(backend().state.settings.behavior.autoCheckIp).toBe(false));
  });

  it("refreshes the rule library and says what arrived", async () => {
    await renderSettings();
    const user = userEvent.setup();

    await user.press(await screen.findByText("Update now"));

    await waitFor(() => expect(screen.getByText("Updated 3 files")).toBeOnTheScreen());
    expect(backend().state.calls.map((call) => call.command)).toEqual(
      expect.arrayContaining(["updateGeoAssets", "updateSrsAssets"]),
    );
    // Both halves land, and the timestamp replaces "never updated".
    expect(screen.queryByText("Not updated from this device yet")).toBeNull();
  });

  it("saves a resolver as it is typed", async () => {
    await renderSettings();
    const user = userEvent.setup();
    const remote = await screen.findByLabelText("Remote DNS");

    await user.clear(remote);
    await user.type(remote, "1.1.1.1");

    // The shared draft debounces and writes; nothing here is a save button.
    await waitFor(() => expect(backend().state.settings.dns.remote).toBe("1.1.1.1"));
    expect(remote.props.value).toBe("1.1.1.1");
  });

  it("switches FakeIP through the same draft", async () => {
    await renderSettings();

    await fireEvent(await screen.findByRole("switch", { name: "FakeIP" }), "valueChange", true);

    await waitFor(() => expect(backend().state.settings.dns.fakeIp).toBe(true));
  });

  it("streams the core log only while it is open, and says when there is none", async () => {
    const { unmount } = await renderSettings();

    await waitFor(() => expect(backend().state.logStreaming).toBe(true));
    expect(
      screen.getByText(
        "Detailed connection logging is off, so only VoyaVPN's own messages appear here. Turn on “Record detailed connection log” above for connection details.",
      ),
    ).toBeOnTheScreen();

    await unmount();
    await waitFor(() => expect(backend().state.logStreaming).toBe(false));
  });
});
