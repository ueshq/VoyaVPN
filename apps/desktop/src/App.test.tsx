import {
  act,
  cleanup,
  fireEvent,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderWithQuery } from "@/test/render";
import { afterEach, beforeAll, vi } from "vitest";

import { App } from "./App";
import { changeLocale } from "@voya/i18n";
import { installFakeCommands, seedListCommands } from "@voya/features/test/backend";
import type {
  ProxyConnectionItem,
  RuntimeStatusResponse,
} from "@voya/contracts";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { usePreferencesStore } from "@voya/client/preferences-store";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@voya/client/toast-store";

vi.mock("@/ipc/window", async (importOriginal) => ({
  // The shell checks for Tauri through the real module; only the window plugin is faked.
  isTauriRuntime: (await importOriginal<typeof import("@/ipc/window")>()).isTauriRuntime,
  onWindowResized: vi.fn(() => Promise.resolve(() => undefined)),
}));
vi.mock("@/ipc/tauri-plugins", () => ({
  check: vi.fn(() => Promise.resolve(null)),
  getVersion: vi.fn(() => Promise.resolve("0.1.0")),
  relaunch: vi.fn(() => Promise.resolve()),
}));

const ipc = installFakeCommands({
  closeWindow: vi.fn(() => Promise.resolve(null)),
  isWindowMaximized: vi.fn(() => Promise.resolve(false)),
  minimizeWindow: vi.fn(() => Promise.resolve(null)),
  toggleMaximizeWindow: vi.fn(() => Promise.resolve(null)),
  connectActiveProfile: vi.fn(),
  appUpdateStatus: vi.fn(() =>
    Promise.resolve({
      currentVersion: "0.1.0",
      state: "unconfigured",
      message: null,
    }),
  ),
  proxyCloseConnection: vi.fn(async () => ({
    connections: [] as ProxyConnectionItem[],
    downloadTotal: 0,
    uploadTotal: 0,
  })),
  proxyListConnections: vi.fn(async () => ({
    connections: [] as ProxyConnectionItem[],
    downloadTotal: 0,
    uploadTotal: 0,
  })),
  proxySetTrafficMode: vi.fn(),
  proxyStartMonitor: vi.fn(() =>
    Promise.resolve({
      state: "running",
      running: true,
      stale: false,
      message: null,
    }),
  ),
  proxyStopMonitor: vi.fn(() =>
    Promise.resolve({
      state: "stopped",
      running: false,
      stale: true,
      message: null,
    }),
  ),
  deleteSubscriptions: vi.fn(),
  deleteProfiles: vi.fn(),
  deleteRoutingRules: vi.fn(),
  deleteRoutings: vi.fn(),
  disconnectCore: vi.fn(),
  generateQrCode: vi.fn(() =>
    Promise.resolve({ mimeType: "image/svg+xml", svg: "<svg />" }),
  ),
  getWindowChromeConfig: vi.fn(() =>
    Promise.resolve({ titleBarLayout: "none" }),
  ),
  importProfilesFromText: vi.fn(),
  loadDnsSettings: vi.fn(() =>
    Promise.resolve({
      addCommonHosts: null,
      blockBindingQuery: null,
      bootstrap: null,
      direct: null,
      directExpectedIps: null,
      directStrategy: null,
      fakeIp: null,
      globalFakeIp: null,
      hosts: null,
      proxyStrategy: null,
      remote: null,
    }),
  ),
  listProcessCandidates: vi.fn(() => Promise.resolve([])),
  ...seedListCommands(),
  policyGroupRuntime: vi.fn(() => Promise.resolve(null)),
  listSubscriptionMetadata: vi.fn(() => Promise.resolve([])),
  getSettingsApplyStatus: vi.fn(async () => ({
    action: "none",
    connected: false,
  })),
  applyPendingSettings: vi.fn(async () => ({
    action: "none",
    connected: false,
  })),
  loadAppSettings: vi.fn(() => new Promise(() => undefined)),
  loadUiPreferences: vi.fn(() =>
    Promise.resolve({ language: "en", theme: "system" }),
  ),
  moveRoutingRule: vi.fn(),
  moveProfile: vi.fn(),
  restartCore: vi.fn(),
  runtimeStatus: vi.fn(),
  saveProfile: vi.fn(),
  saveRouting: vi.fn(),
  saveRoutingRule: vi.fn(),
  saveAppSettings: vi.fn(),
  saveAppConfig: vi.fn((config) => Promise.resolve(config)),
  saveUiPreferences: vi.fn((preferences) => Promise.resolve(preferences)),
  saveDnsSettings: vi.fn(),
  saveSubscription: vi.fn(),
  setActiveProfile: vi.fn(),
  setActiveRouting: vi.fn(),
  setAutostartEnabled: vi.fn((enabled) =>
    Promise.resolve({
      artifactKind: "linuxDesktopFile",
      artifactName: "VoyaVPN.desktop",
      artifactPath: "/home/test/.config/autostart/VoyaVPN.desktop",
      enabled,
      platform: "linux",
    }),
  ),
  setConnectionMode: vi.fn(() =>
    Promise.resolve({
      mode: "systemProxy",
      processRulesEffective: false,
      processRulesSupported: true,
      systemProxyAvailable: true,
      vpnAvailable: true,
    }),
  ),
  setWindowAcrylic: vi.fn(() => Promise.resolve(null)),
  speedtestStatus: vi.fn(() => Promise.resolve({ running: false })),
  tunRequestElevation: vi.fn(),
  systemProxyStatus: vi.fn(() =>
    Promise.resolve({
      management: "automatic",
      effectiveMode: "forcedClear",
      exceptions: "",
      proxy: null,
      requestedMode: "forcedClear",
    }),
  ),
  tunStatus: vi.fn(() =>
    Promise.resolve({
      allowEnableTun: true,
      backend: "process",
      enabled: false,
      elevationGranted: false,
      lastProviderError: null,
      nativeComponentReady: true,
      needsServiceInstall: false,
      needsVpnPermission: false,
      preflight: {
        notes: [],
        platform: "linux",
        routeRestoreNote: "",
        state: "ready",
        windowsCleanupDevices: [],
      },
      expectedProviderPath: null,
      providerPathMismatch: false,
      providerState: "notApplicable",
      requiresElevation: false,
      resolvedProviderPath: null,
      restoreOnDisconnect: true,
    }),
  ),
  updateSrsAssets: vi.fn(() => Promise.resolve([])),
  updateSubscriptions: vi.fn(),
});
vi.mock("@/ipc/event-bridge", () => ({ EventBridge: () => null }));

function renderApp() {
  return renderWithQuery(<App />);
}

describe("App", () => {
  // Coverage transformation of real lazy screens is setup work, not UI
  // latency. Finish it before Testing Library starts its interaction timeout.
  beforeAll(async () => {
    await Promise.all([
      import("@/features/home/home-screen"),
      import("@/features/settings/settings-screen"),
    ]);
  }, 30000);
  beforeEach(async () => {
    vi.useRealTimers();
    resetTestDom();
    useShellStore.setState({
      activeTab: "profiles",
      connectionSearch: "",
      connectionsView: "connections",
      sidebarCollapsed: false,
    });
    useToastStore.setState({ toasts: [] });
    usePreferencesStore.setState({ themeMode: "system" });
    window.history.replaceState({}, "", "/");
    window.localStorage.clear();
    document.documentElement.className = "";
    ipc.loadUiPreferences.mockReset();
    ipc.loadUiPreferences.mockResolvedValue({
      language: "en",
      theme: "system",
    });
    ipc.getWindowChromeConfig.mockResolvedValue({
      titleBarLayout: "none",
    });
    ipc.runtimeStatus.mockResolvedValue(disconnectedCore());
    ipc.proxyCloseConnection.mockClear();
    ipc.proxyListConnections.mockClear();
    ipc.proxyStartMonitor.mockClear();
    ipc.proxyStopMonitor.mockClear();
    ipc.proxyCloseConnection.mockResolvedValue({
      connections: [],
      downloadTotal: 0,
      uploadTotal: 0,
    });
    ipc.proxyListConnections.mockResolvedValue({
      connections: [],
      downloadTotal: 0,
      uploadTotal: 0,
    });
    ipc.proxyStartMonitor.mockResolvedValue({
      state: "running",
      running: true,
      stale: false,
      message: null,
    });
    ipc.proxyStopMonitor.mockResolvedValue({
      state: "stopped",
      running: false,
      stale: true,
      message: null,
    });
    await changeLocale("en");
  });

  afterEach(() => {
    vi.useRealTimers();
    resetTestDom();
    delete (window as typeof window & { __TAURI_INTERNALS__?: unknown })
      .__TAURI_INTERNALS__;
  });

  it("renders the six-item sidebar nav with the speed footer", () => {
    renderApp();

    const sidebar = screen.getByRole("complementary");
    const footer = screen.getByTestId("sidebar-footer");
    const tablist = screen.getByRole("tablist", { name: "Main sections" });

    const tabNames = within(tablist)
      .getAllByRole("tab")
      .map((tab) => tab.textContent);
    expect(tabNames).toEqual([
      "Home",
      "Nodes",
      "Rules",
      "Network activity",
      "Self-hosted node",
      "Settings",
    ]);
    expect(footer).toHaveTextContent("Disconnected");
    // Rates appear only while connected; a disconnected footer is just its state.
    expect(footer).not.toHaveTextContent("B/s");
    expect(screen.queryByTestId("status-bar")).not.toBeInTheDocument();
    expect(
      within(sidebar).queryByRole("button", { name: "Settings" }),
    ).toBeNull();
    expect(within(sidebar).queryByRole("button", { name: "Theme" })).toBeNull();
  });

  it("collapses the sidebar without losing navigation or accessible labels", async () => {
    const user = userEvent.setup();
    renderApp();
    await user.click(screen.getByRole("button", { name: "Collapse sidebar" }));
    expect(
      screen.getByRole("button", { name: "Expand sidebar" }),
    ).toHaveAttribute("aria-expanded", "false");
    expect(
      within(
        screen.getByRole("tablist", { name: "Main sections" }),
      ).getAllByRole("tab"),
    ).toHaveLength(6);
    await user.click(screen.getByRole("tab", { name: "Settings" }));
    expect(
      await screen.findByRole("region", { name: "Settings" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Expand sidebar" }));
    expect(
      screen.getByRole("button", { name: "Collapse sidebar" }),
    ).toHaveAttribute("aria-expanded", "true");
  });

  it.each(["macos", "windows", "none"] as const)(
    "uses %s chrome without a separate titlebar row",
    async (layout) => {
      ipc.getWindowChromeConfig.mockResolvedValue({
        titleBarLayout: layout,
      });
      const { container } = renderApp();
      await waitFor(() =>
        expect(container.querySelector(".app-shell")).toHaveAttribute(
          "data-window-chrome",
          layout,
        ),
      );
      const toolbar = container.querySelector(".sidebar-toolbar");
      const toggle = screen.getByRole("button", { name: "Collapse sidebar" });
      expect(toggle).not.toHaveAttribute("data-tauri-drag-region");
      if (layout === "none") {
        expect(toolbar).not.toHaveAttribute("data-tauri-drag-region");
        expect(container.querySelector('[data-slot="titlebar"]')).toBeNull();
      } else {
        expect(toolbar).toHaveAttribute("data-tauri-drag-region");
        expect(
          container.querySelector(
            '.shell-content-column > [data-slot="titlebar"]',
          ),
        ).toBeInTheDocument();
      }
      expect(
        screen.queryAllByRole("button", { name: "Minimize" }),
      ).toHaveLength(layout === "windows" ? 1 : 0);
      expect(
        container.querySelector('[data-slot="titlebar-placeholder"]'),
      ).toBeNull();
      // No app name or logo beside the traffic lights, and no titlebar row carrying one.
      expect(toolbar).not.toHaveTextContent("VoyaVPN");
      expect(toolbar?.querySelector("svg:not(.lucide)")).toBeNull();
      expect(
        container.querySelector('[data-slot="titlebar"]')?.textContent ?? "",
      ).not.toContain("VoyaVPN");
    },
  );

  it("defaults to the connection home hero", async () => {
    useShellStore.setState({ activeTab: "home" });

    renderApp();

    const hero = await screen.findByRole("region", { name: "Connection home" });
    expect(
      within(hero).queryByRole("heading", { level: 1 }),
    ).not.toBeInTheDocument();
    expect(
      await within(hero).findByRole("button", { name: "Add node" }),
    ).toBeInTheDocument();
    expect(within(hero).queryByText("Not protected")).not.toBeInTheDocument();
    expect(within(hero).queryByRole("switch")).not.toBeInTheDocument();
    // The traffic mode lives on the Rules page.
    expect(within(hero).queryByRole("group", { name: "Traffic mode" })).not.toBeInTheDocument();
    expect(screen.getByTestId("sidebar-footer")).toHaveTextContent(
      "Disconnected",
    );
  });

  it("falls back to a supported locale when the backend stores a removed language", async () => {
    await changeLocale("zh-Hans", { persist: false });
    ipc.loadUiPreferences.mockResolvedValue({
      language: "fa",
      theme: "system",
    });
    renderApp();

    await waitFor(() =>
      expect(document.documentElement).toHaveAttribute("lang", "en"),
    );
    expect(document.documentElement).toHaveAttribute("dir", "ltr");
    expect(mainNavTab(/Nodes/)).toBeInTheDocument();
  });

  it("hydrates the theme through the dedicated preferences query", async () => {
    ipc.loadUiPreferences.mockResolvedValue({
      language: "en",
      theme: "dark",
    });

    renderApp();

    await waitFor(() => expect(document.documentElement).toHaveClass("dark"));
  });

  it("opens the in-shell settings screen from the sidebar tab", async () => {
    renderApp();

    await activateTab(/Settings/);

    expect(
      await screen.findByRole("region", { name: "Settings" }),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole("tab", { name: "General", selected: true }),
    ).toBeInTheDocument();
    expect(screen.getByRole("complementary")).toBeInTheDocument();
  });

  it("starts connection queries and the debounced monitor only after the core connects", async () => {
    await import("@/features/proxy/connections-screen");
    vi.useFakeTimers();
    (
      window as typeof window & { __TAURI_INTERNALS__?: unknown }
    ).__TAURI_INTERNALS__ = {};
    useRuntimeEventStore.getState().setCoreState({
      ...connectedCore(),
      state: "disconnected",
    });
    renderApp();
    await activateTab(/Network activity/);
    expect(
      screen.getByRole("heading", { name: "Network activity" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Connect to view network activity"),
    ).toBeInTheDocument();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });
    expect(ipc.proxyStartMonitor).not.toHaveBeenCalled();
    expect(ipc.proxyListConnections).not.toHaveBeenCalled();

    await activateTab(/Nodes/);
    connectCore();
    await activateTab(/Network activity/);
    expect(ipc.proxyListConnections).toHaveBeenCalledTimes(1);
    expect(ipc.proxyStartMonitor).not.toHaveBeenCalled();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(ipc.proxyStartMonitor).toHaveBeenCalledTimes(1);
    expect(useRuntimeEventStore.getState().proxyMonitorStatus.state).toBe(
      "running",
    );
    await activateTab(/Nodes/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_999);
    });
    expect(ipc.proxyStopMonitor).not.toHaveBeenCalled();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(ipc.proxyStopMonitor).toHaveBeenCalledTimes(1);
    expect(useRuntimeEventStore.getState().proxyMonitorStatus.state).toBe(
      "stopped",
    );
  });

  it("stops the monitor while the window is hidden into the tray and resumes when shown", async () => {
    await import("@/features/proxy/connections-screen");
    vi.useFakeTimers();
    (
      window as typeof window & { __TAURI_INTERNALS__?: unknown }
    ).__TAURI_INTERNALS__ = {};
    connectCore();
    let visibility: DocumentVisibilityState = "visible";
    const visibilityState = vi
      .spyOn(document, "visibilityState", "get")
      .mockImplementation(() => visibility);
    const setVisibility = async (next: DocumentVisibilityState) => {
      visibility = next;
      await act(async () => {
        document.dispatchEvent(new Event("visibilitychange"));
      });
    };

    try {
      renderApp();
      await activateTab(/Network activity/);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(100);
      });
      expect(ipc.proxyStartMonitor).toHaveBeenCalledTimes(1);

      await setVisibility("hidden");
      await act(async () => {
        await vi.advanceTimersByTimeAsync(2_000);
      });
      expect(ipc.proxyStopMonitor).toHaveBeenCalledTimes(1);

      await setVisibility("visible");
      await act(async () => {
        await vi.advanceTimersByTimeAsync(100);
      });
      expect(ipc.proxyStartMonitor).toHaveBeenCalledTimes(2);
    } finally {
      visibilityState.mockRestore();
    }
  });

  it("does not start the proxy monitor on Nodes even when connected", async () => {
    vi.useFakeTimers();
    (
      window as typeof window & { __TAURI_INTERNALS__?: unknown }
    ).__TAURI_INTERNALS__ = {};
    connectCore();
    renderApp();
    await activateTab(/Nodes/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(ipc.proxyStartMonitor).not.toHaveBeenCalled();
    expect(ipc.proxyListConnections).not.toHaveBeenCalled();
  });

  it("keeps the proxy monitor running while viewing the policy group sub-tab", async () => {
    vi.useFakeTimers();
    (
      window as typeof window & { __TAURI_INTERNALS__?: unknown }
    ).__TAURI_INTERNALS__ = {};

    connectCore();
    renderApp();

    await activateTab(/Network activity/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(ipc.proxyStartMonitor).toHaveBeenCalledTimes(1);

    const page = screen.getByRole("region", { name: "Network activity" });
    const groupsTab = within(page).getByRole("tab", { name: "Policy groups" });
    await act(async () => {
      fireEvent.mouseDown(groupsTab);
      fireEvent.click(groupsTab);
      await Promise.resolve();
    });

    expect(groupsTab).toHaveAttribute("data-state", "active");
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000);
    });
    expect(ipc.proxyStopMonitor).not.toHaveBeenCalled();
  });

  it("marks cached proxy monitor data failed and shows a toast when start fails", async () => {
    vi.useFakeTimers();
    (
      window as typeof window & { __TAURI_INTERNALS__?: unknown }
    ).__TAURI_INTERNALS__ = {};
    ipc.proxyStartMonitor.mockRejectedValueOnce(
      new Error("start unavailable"),
    );

    connectCore();
    renderApp();

    await activateTab(/Network activity/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });

    expect(ipc.proxyStartMonitor).toHaveBeenCalledTimes(1);
    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: "start unavailable",
      running: false,
      stale: true,
      state: "failed",
    });
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "start unavailable",
      title: "Proxy runtime",
    });
  });

  it("marks cached proxy monitor data failed and shows a toast when delayed stop fails", async () => {
    vi.useFakeTimers();
    (
      window as typeof window & { __TAURI_INTERNALS__?: unknown }
    ).__TAURI_INTERNALS__ = {};
    ipc.proxyStopMonitor.mockRejectedValueOnce(
      new Error("stop unavailable"),
    );

    connectCore();
    renderApp();

    await activateTab(/Network activity/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(useRuntimeEventStore.getState().proxyMonitorStatus.state).toBe(
      "running",
    );

    await activateTab(/Home/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000);
    });

    expect(ipc.proxyStopMonitor).toHaveBeenCalledTimes(1);
    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: "stop unavailable",
      running: false,
      stale: true,
      state: "failed",
    });
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "stop unavailable",
      title: "Proxy runtime",
    });
  });

  it("consolidates failed updates and keeps stale data explicit after a manual refresh", async () => {
    const user = userEvent.setup();
    connectCore();
    useRuntimeEventStore.getState().setProxyMonitorFailed("monitor offline");
    const cachedSnapshot = {
      connections: [makeConnection(0, { host: "cached.example:443" })],
      downloadTotal: 100,
      uploadTotal: 50,
    };
    const refreshedSnapshot = {
      connections: [makeConnection(1, { host: "fresh.example:443" })],
      downloadTotal: 4096,
      uploadTotal: 1024,
    };
    useRuntimeEventStore.getState().setProxyConnections(cachedSnapshot);
    ipc.proxyListConnections
      .mockResolvedValueOnce(cachedSnapshot)
      .mockResolvedValueOnce(refreshedSnapshot);
    renderApp();
    await user.click(mainNavTab(/Network activity/));
    await waitFor(() => expect(ipc.proxyListConnections).toHaveBeenCalledTimes(1));
    expect(
      screen.getAllByText("Unable to update connections right now"),
    ).toHaveLength(1);
    expect(screen.queryByText("monitor offline")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Refresh list" }));
    expect(await screen.findByText("fresh.example:443")).toBeInTheDocument();
    expect(useRuntimeEventStore.getState().proxyConnections).toMatchObject(refreshedSnapshot);
    expect(screen.getByText("Showing previous data")).toBeInTheDocument();
    expect(useRuntimeEventStore.getState().proxyMonitorStatus.state).toBe("failed");
  });

  it("keeps the connection search between sub-tabs and after leaving the page", async () => {
    const user = userEvent.setup();
    connectCore();
    useRuntimeEventStore.getState().setProxyMonitorStatus({
      message: null,
      running: true,
      stale: false,
      state: "running",
    });
    ipc.proxyListConnections.mockResolvedValue({
      connections: makeConnections(2),
      downloadTotal: 2,
      uploadTotal: 1,
    });
    renderApp();
    await user.click(mainNavTab(/Network activity/));
    const page = screen.getByRole("region", { name: "Network activity" });
    await user.type(within(page).getByRole("searchbox"), "bulk-1");
    await user.click(within(page).getByRole("tab", { name: "Policy groups" }));
    await user.click(
      within(page).getByRole("tab", { name: "Live connections" }),
    );
    expect(within(page).getByRole("searchbox")).toHaveValue("bulk-1");
    await user.click(within(page).getByRole("tab", { name: "Policy groups" }));
    await user.click(mainNavTab(/Nodes/));
    await user.click(mainNavTab(/Network activity/));
    expect(
      within(
        screen.getByRole("region", { name: "Network activity" }),
      ).getByRole("tab", { name: "Policy groups" }),
    ).toHaveAttribute("data-state", "active");
    await user.click(screen.getByRole("tab", { name: "Live connections" }));
    // A visit to another page no longer throws the search away.
    expect(screen.getByRole("searchbox")).toHaveValue("bulk-1");
  });
});

function mainNavTab(name: RegExp) {
  return within(
    screen.getByRole("tablist", { name: "Main sections" }),
  ).getByRole("tab", { name });
}

async function activateTab(name: RegExp) {
  await act(async () => {
    fireEvent.click(mainNavTab(name));
    await Promise.resolve();
  });
}

function resetTestDom() {
  cleanup();
  document.body.innerHTML = "";
  document.body.removeAttribute("data-scroll-locked");
  document.body.style.removeProperty("pointer-events");
}

function makeConnection(
  index: number,
  overrides: Partial<ProxyConnectionItem> = {},
): ProxyConnectionItem {
  return {
    chains: ["Proxy"],
    connectionType: "HTTP",
    destination: "93.184.216.34:443",
    download: index,
    host: `bulk-${index}.example:443`,
    id: `connection-${index}`,
    network: "tcp",
    process: "browser",
    processPath: "/usr/bin/browser",
    rule: "MATCH",
    rulePayload: null,
    source: "127.0.0.1:53000",
    start: "2026-06-01T00:00:00Z",
    upload: index,
    ...overrides,
  };
}

function makeConnections(count: number): ProxyConnectionItem[] {
  return Array.from({ length: count }, (_, index) => makeConnection(index));
}

/**
 * Puts the core in `connected` both in the store and in what the backend
 * reports, so the shell's own status refresh does not undo it.
 */
function connectCore() {
  ipc.runtimeStatus.mockResolvedValue(connectedCore());
  useRuntimeEventStore.getState().setCoreState(connectedCore());
}

function disconnectedCore(): RuntimeStatusResponse {
  return {
    state: "disconnected",
    activeProfileId: null,
    mainPid: null,
    prePid: null,
    connectedDurationMs: null,
    activeTunBackend: null,
  };
}

function connectedCore(): RuntimeStatusResponse {
  return {
    state: "connected",
    activeProfileId: null,
    mainPid: 42,
    prePid: null,
    connectedDurationMs: 0,
    activeTunBackend: null,
  };
}
