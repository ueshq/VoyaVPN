import { act, render, screen, userEvent, waitFor } from "@testing-library/react-native";
import type { QueryClient } from "@tanstack/react-query";
import type { MockBackend } from "@voya/client/mock-backend";
import { usePreferencesStore } from "@voya/client/preferences-store";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { i18nHost } from "@voya/i18n/core";

import { registerMobileBackend, voyaTransport } from "~/ipc/platform";
import { localeReady } from "~/native/platform-boot";
import { makeTestQueryClient, TestProviders } from "~/test/providers";

import { GeneralScreen, MaintenanceScreen } from "./preferences-screens";
import { DnsScreen } from "./dns-screen";
import { LogsScreen } from "./logs-screen";

let activeQueryClient: QueryClient | null = null;

async function renderSettings(section: "general" | "dns" | "maintenance" | "logs" = "general") {
  const Screen = section === "dns" ? DnsScreen : section === "maintenance" ? MaintenanceScreen : section === "logs" ? LogsScreen : GeneralScreen;
  const queryClient = makeTestQueryClient();
  activeQueryClient = queryClient;
  const wrapper = ({ children }: { children: React.ReactNode }) => (
    <TestProviders queryClient={queryClient}>{children}</TestProviders>
  );

  return { queryClient, ...(await render(<Screen />, { wrapper })) };
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
    const user = userEvent.setup();

    // HeroUI's Switch is a Pressable; a press is the toggle.
    await user.press(
      await screen.findByRole("switch", {
        name: "Check the exit IP after connecting",
      }),
    );

    await waitFor(() => expect(backend().state.settings.behavior.autoCheckIp).toBe(false));
  });

  it("refreshes the rule library and says what arrived", async () => {
    await renderSettings("maintenance");
    const user = userEvent.setup();

    await user.press(await screen.findByText("Update now"));

    await waitFor(() => expect(screen.getByText("Updated 3 files")).toBeOnTheScreen());
    expect(backend().state.calls.map((call) => call.command)).toEqual(
      expect.arrayContaining(["updateGeoAssets", "updateSrsAssets"]),
    );
    // Both halves land, and the timestamp replaces "never updated".
    expect(screen.queryByText("Not updated from this device yet")).toBeNull();
  });

  it("keeps DNS edits local until Save and preserves hidden fields", async () => {
    backend().state.settings.dns.hosts = "127.0.0.1 internal.test";
    await renderSettings("dns");
    const user = userEvent.setup();
    await user.press(await screen.findByText("Custom & advanced"));
    const remote = await screen.findByLabelText("Remote DNS");

    await user.clear(remote);
    await user.type(remote, "1.1.1.1");

    expect(backend().state.settings.dns.remote).not.toBe("1.1.1.1");
    await user.press(screen.getByText("Save"));
    await waitFor(() => expect(backend().state.settings.dns.remote).toBe("1.1.1.1"));
    expect(backend().state.settings.dns.hosts).toBe("127.0.0.1 internal.test");
  });

  it("restores only DNS fields owned by this page and still requires Save", async () => {
    backend().state.settings.dns.hosts = "127.0.0.1 retained.test";
    backend().state.settings.dns.remote = "9.9.9.9";
    await renderSettings("dns");
    const user = userEvent.setup();
    await user.press(await screen.findByText("Restore DNS defaults"));
    expect(backend().state.settings.dns.remote).toBe("9.9.9.9");
    await user.press(screen.getByText("Save"));
    await waitFor(() => expect(backend().state.settings.dns.remote).not.toBe("9.9.9.9"));
    expect(backend().state.settings.dns.hosts).toBe("127.0.0.1 retained.test");
  });

  it("distinguishes saved DNS from an unsuccessful apply and allows retry", async () => {
    const status = jest.spyOn(backend().commands, "getSettingsApplyStatus").mockResolvedValue({ action: "reconnect", connected: true });
    const apply = jest.spyOn(backend().commands, "applyPendingSettings")
      .mockRejectedValueOnce(new Error("reconnect failed"))
      .mockImplementationOnce(async () => {
        status.mockResolvedValue({ action: "none", connected: true });
        return { action: "none", connected: true };
      });
    await renderSettings("dns");
    const user = userEvent.setup();
    await user.press(await screen.findByText("Reconnect & apply"));
    expect(await screen.findByText("Settings saved, but they could not be applied to the current connection")).toBeOnTheScreen();
    expect(screen.getByText("Saved")).toBeOnTheScreen();
    expect(screen.queryByText("Applied to the current connection")).toBeNull();
    await user.press(screen.getByText("Reconnect & apply"));
    expect(await screen.findByText("Applied to the current connection")).toBeOnTheScreen();
    expect(screen.queryByText("Settings saved, but they could not be applied to the current connection")).toBeNull();
    status.mockRestore(); apply.mockRestore();
  });

  it("saves advanced DNS switches explicitly", async () => {
    await renderSettings("dns");
    const user = userEvent.setup();

    await user.press(await screen.findByText("Custom & advanced"));
    await user.press(await screen.findByRole("switch", { name: "FakeIP" }));
    expect(backend().state.settings.dns.fakeIp).toBe(false);
    await user.press(screen.getByText("Save"));

    await waitFor(() => expect(backend().state.settings.dns.fakeIp).toBe(true));
  });

  it("derives log status from the saved switch and only core log sources", async () => {
    backend().state.settings.core.logEnabled = true;
    await renderSettings("maintenance");
    expect(await screen.findByText("Detailed connection logging is enabled; no core logs yet.")).toBeOnTheScreen();
    for (const source of ["diagnostic", "app", "core"] as const) {
      await act(() => useRuntimeEventStore.setState({ logLines: [{
        id: 1, loggedAt: Date.now(), level: "info", body: source === "app"
          ? { source, code: { code: "disconnected" }, detail: null }
          : { source, line: "history" },
      }] }));
      expect(screen.getByText(source === "core"
        ? "Detailed connection logging is enabled; core logs have been received."
        : "Detailed connection logging is enabled; no core logs yet.")).toBeOnTheScreen();
    }
  });

  it("streams logs only while the log page is open", async () => {
    const { unmount } = await renderSettings("logs");
    await waitFor(() => expect(backend().state.logStreaming).toBe(true));
    expect(screen.getByText(/Keeps the latest 500 entries/)).toBeOnTheScreen();
    await unmount();
    await waitFor(() => expect(backend().state.logStreaming).toBe(false));
  });
});
