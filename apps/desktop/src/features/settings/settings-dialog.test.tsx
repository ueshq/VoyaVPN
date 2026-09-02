import type { ReactNode } from "react";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import { useShellStore } from "@/stores/shell-store";
import { makeAppSettings } from "./app-settings.test-fixture";
import { SettingsSurface } from "./settings-dialog";
import { SettingsScreen } from "./settings-screen";

const ipcMocks = vi.hoisted(() => ({
  IpcCommandError: class MockIpcCommandError extends Error {},
  appUpdateStatus: vi.fn(),
  loadAppSettings: vi.fn(),
  loadDnsSettings: vi.fn(),
  saveAppSettings: vi.fn(),
  saveDnsSettings: vi.fn(),
  updateGeoAssets: vi.fn(),
  updateSrsAssets: vi.fn(),
}));
const updaterMocks = vi.hoisted(() => ({
  check: vi.fn(),
  getVersion: vi.fn(),
}));

vi.mock("@/ipc", () => ipcMocks);
vi.mock("@/ipc/process", () => ({ relaunch: vi.fn() }));
vi.mock("@/ipc/updater", () => updaterMocks);

describe("unified settings surface", () => {
  beforeEach(async () => {
    cleanup();
    vi.clearAllMocks();
    await changeLocale("en");
    ipcMocks.appUpdateStatus.mockResolvedValue({ currentVersion: "0.1.0", message: null, state: "ready" });
    ipcMocks.loadAppSettings.mockResolvedValue(makeAppSettings());
    ipcMocks.loadDnsSettings.mockResolvedValue({
      addCommonHosts: null,
      blockBindingQuery: null,
      bootstrap: null,
      direct: null,
      directExpectedIps: null,
      directStrategy: null,
      fakeIp: null,
      globalFakeIp: null,
      hosts: null,
      parallelQuery: null,
      proxyStrategy: null,
      remote: null,
      serveStale: null,
      useSystemHosts: null,
    });
    ipcMocks.saveAppSettings.mockImplementation(async (settings) => settings);
    ipcMocks.saveDnsSettings.mockImplementation(async (settings) => settings);
    ipcMocks.updateGeoAssets.mockResolvedValue([]);
    ipcMocks.updateSrsAssets.mockResolvedValue([]);
    updaterMocks.check.mockResolvedValue(null);
    updaterMocks.getVersion.mockResolvedValue("0.1.0");
    useShellStore.setState({
      activeTab: "settings",
      navigationGuard: null,
      pendingTab: null,
    });
  });

  afterEach(cleanup);

  it("keeps one draft across tabs and exposes no Hotkeys tab", async () => {
    const user = userEvent.setup();
    renderSurface();

    expect(await screen.findByRole("tab", { name: "General", selected: true })).toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Hotkeys" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Core" }));
    const userAgent = await screen.findByDisplayValue("agent-before-edit");
    await user.clear(userAgent);
    await user.type(userAgent, "agent-after-edit");
    await user.click(screen.getByRole("tab", { name: "Network" }));
    expect(await screen.findByLabelText("MTU")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Core" }));

    expect(screen.getByDisplayValue("agent-after-edit")).toBeInTheDocument();
    expect(screen.getByText("Unsaved changes")).toBeInTheDocument();
    expect(ipcMocks.loadAppSettings).toHaveBeenCalledTimes(1);
  });

  it("persists all edits through the single Save all action", async () => {
    const user = userEvent.setup();
    renderSurface();
    await user.click(await screen.findByRole("tab", { name: "Core" }));
    const userAgent = await screen.findByDisplayValue("agent-before-edit");
    await user.clear(userAgent);
    await user.type(userAgent, "saved-agent");

    await user.click(screen.getByRole("button", { name: "Save all" }));

    await waitFor(() => expect(ipcMocks.saveAppSettings).toHaveBeenCalledTimes(1));
    expect(ipcMocks.saveAppSettings).toHaveBeenCalledWith(
      expect.objectContaining({
        core: expect.objectContaining({ defaultUserAgent: "saved-agent" }),
      }),
    );
  });

  it("hosts the DNS pane as a settings tab with its own save action", async () => {
    const user = userEvent.setup();
    renderSurface();

    await user.click(await screen.findByRole("tab", { name: "DNS" }));

    expect(await screen.findByLabelText("Remote DNS")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 2, name: "DNS" })).toBeInTheDocument();
    expect(ipcMocks.loadDnsSettings).toHaveBeenCalledTimes(1);
  });

  it("renders the in-shell settings screen and navigates away freely while clean", async () => {
    renderScreen();
    await screen.findByRole("region", { name: "Settings" });
    expect(screen.getByRole("heading", { level: 1, name: "Settings" })).toBeInTheDocument();
    expect(await screen.findByRole("tab", { name: "General", selected: true })).toBeInTheDocument();

    act(() => useShellStore.getState().requestTab("home"));

    expect(useShellStore.getState().activeTab).toBe("home");
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("guards navigation while settings are dirty and honors cancel", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.click(await screen.findByRole("button", { name: "Light" }));

    act(() => useShellStore.getState().requestTab("home"));

    expect(await screen.findByRole("alertdialog")).toHaveTextContent("Unsaved settings");
    expect(useShellStore.getState().activeTab).toBe("settings");

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(useShellStore.getState().activeTab).toBe("settings");
    expect(useShellStore.getState().pendingTab).toBeNull();
  });

  it("discards dirty settings and completes the blocked navigation", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.click(await screen.findByRole("button", { name: "Light" }));

    act(() => useShellStore.getState().requestTab("home"));
    const dialog = within(await screen.findByRole("alertdialog"));
    await user.click(dialog.getByRole("button", { name: "Discard changes" }));

    await waitFor(() => expect(useShellStore.getState().activeTab).toBe("home"));
  });

  it("saves dirty settings and completes the blocked navigation", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.click(await screen.findByRole("button", { name: "Light" }));

    act(() => useShellStore.getState().requestTab("home"));
    const dialog = within(await screen.findByRole("alertdialog"));
    await user.click(dialog.getByRole("button", { name: "Save all" }));

    await waitFor(() => expect(ipcMocks.saveAppSettings).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(useShellStore.getState().activeTab).toBe("home"));
  });

  it("keeps the settings tab active when saving dirty settings fails", async () => {
    const user = userEvent.setup();
    ipcMocks.saveAppSettings.mockRejectedValueOnce(new Error("save failed"));
    renderScreen();
    await user.click(await screen.findByRole("button", { name: "Light" }));

    act(() => useShellStore.getState().requestTab("home"));
    const dialog = within(await screen.findByRole("alertdialog"));
    await user.click(dialog.getByRole("button", { name: "Save all" }));

    await waitFor(() => expect(ipcMocks.saveAppSettings).toHaveBeenCalledTimes(1));
    expect(useShellStore.getState().activeTab).toBe("settings");
    expect(screen.getByRole("alertdialog")).toBeVisible();
  });
});

function renderSurface() {
  return renderWithQuery(<SettingsSurface />);
}

function renderScreen() {
  return renderWithQuery(<SettingsScreen />);
}

function renderWithQuery(children: ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}>{children}</QueryClientProvider>);
}
