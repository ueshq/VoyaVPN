import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, vi } from "vitest";

import { App } from "./App";
import { changeLocale } from "@voya/i18n";
import {
  proxyCloseConnection,
  proxyListConnections,
  proxyStartMonitor,
  proxyStopMonitor,
  loadUiPreferences,
} from "@/ipc";
import type {
  ProxyConnectionItem,
  ProxyConnectionsSnapshot,
  SpeedtestStatus,
} from "@/ipc/bindings";
import type { RuntimeEventState, RuntimeProxyMonitorStatus } from "@/ipc/runtime-event-store";
import { usePreferencesStore } from "@/stores/preferences-store";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";

vi.mock("@/ipc/process", () => ({
  relaunch: vi.fn(() => Promise.resolve()),
}));
vi.mock("@/ipc/window", () => ({
  closeWindow: vi.fn(() => Promise.resolve()),
  isWindowMaximized: vi.fn(() => Promise.resolve(false)),
  minimizeWindow: vi.fn(() => Promise.resolve()),
  onWindowResized: vi.fn(() => Promise.resolve(() => undefined)),
  toggleMaximizeWindow: vi.fn(() => Promise.resolve()),
}));
vi.mock("@/ipc/updater", () => ({
  check: vi.fn(() => Promise.resolve(null)),
  getVersion: vi.fn(() => Promise.resolve("0.1.0")),
}));

// The double is checked against the real store type (`satisfies` in `makeState`),
// so a member added to `RuntimeEventState` cannot silently go unmodelled here and
// leave these tests asserting the fake's own bookkeeping.
type TestProxyMonitorState = RuntimeProxyMonitorStatus["state"];

type TestProxyMonitorStatus = RuntimeProxyMonitorStatus;

type TestRuntimeEventStore = {
  getState: () => RuntimeEventState;
  reset: () => void;
  useRuntimeEventStore: {
    (selector: (state: RuntimeEventState) => unknown): unknown;
    getState: () => RuntimeEventState;
  };
};

const runtimeStoreMock = vi.hoisted<TestRuntimeEventStore>(() => {
  const initialMonitorStatus: TestProxyMonitorStatus = {
    message: null,
    running: false,
    stale: true,
    state: "stopped",
  };
  let state: RuntimeEventState;

  function makeMonitorStatus(
    monitorState: TestProxyMonitorState,
    running: boolean,
    stale: boolean,
    message: string | null,
  ): TestProxyMonitorStatus {
    return { message, running, stale, state: monitorState };
  }

  function makeState(): RuntimeEventState {
    const nextState = {
      clearLogs: vi.fn(),
      proxyConnections: null,
      proxyMonitorStatus: initialMonitorStatus,
      coreState: null,
      lastTransientEvent: null,
      logLines: [],
      pushTransientEvent: vi.fn(),
      refreshSpeedtestStatus: vi.fn(() => Promise.resolve()),
      serverStatsByProfileId: {},
      setProxyConnections: vi.fn((snapshot: ProxyConnectionsSnapshot) => {
        state.proxyConnections = snapshot;
      }),
      setProxyMonitorFailed: vi.fn((message: string | null = null) => {
        state.proxyMonitorStatus = makeMonitorStatus("failed", false, true, message);
      }),
      setProxyMonitorRunning: vi.fn((message: string | null = null) => {
        state.proxyMonitorStatus = makeMonitorStatus("running", true, false, message);
      }),
      setProxyMonitorStarting: vi.fn((message: string | null = null) => {
        state.proxyMonitorStatus = makeMonitorStatus(
          "starting",
          false,
          state.proxyMonitorStatus.stale,
          message,
        );
      }),
      setProxyMonitorStatus: vi.fn((status: TestProxyMonitorStatus) => {
        state.proxyMonitorStatus = status;
      }),
      setProxyMonitorStopped: vi.fn((message: string | null = null) => {
        state.proxyMonitorStatus = makeMonitorStatus("stopped", false, true, message);
      }),
      setCoreState: vi.fn(),
      setSpeedtestRunning: vi.fn((speedtestRunning: boolean) => {
        state.speedtestRunning = speedtestRunning;
      }),
      setSpeedtestStatus: vi.fn((status: SpeedtestStatus) => {
        state.speedtestRunning = status.running;
      }),
      setSysProxy: vi.fn(),
      setTun: vi.fn(),
      speedtestResultsByProfileId: {},
      speedtestRunning: false,
      statistics: null,
      sysProxy: null,
      tun: null,
    } satisfies RuntimeEventState;

    return nextState;
  }

  state = makeState();

  const useRuntimeEventStore = Object.assign(
    vi.fn((selector: (state: RuntimeEventState) => unknown) => selector(state)),
    {
      getState: vi.fn(() => state),
    },
  );

  return {
    getState: () => state,
    reset: () => {
      state = makeState();
      useRuntimeEventStore.mockClear();
      useRuntimeEventStore.getState.mockClear();
    },
    useRuntimeEventStore,
  };
});

vi.mock("@/ipc", () => ({
  connectActiveProfile: vi.fn(),
  EventBridge: () => null,
  appUpdateStatus: vi.fn(() => Promise.resolve({ currentVersion: "0.1.0", state: "unconfigured", message: null })),
  proxyCloseConnection: vi.fn(() => Promise.resolve({ connections: [], downloadTotal: 0, uploadTotal: 0 })),
  proxyListConnections: vi.fn(() => Promise.resolve({ connections: [], downloadTotal: 0, uploadTotal: 0 })),
  proxyListGroups: vi.fn(() => Promise.resolve({ groups: [], trafficMode: "rule" })),
  proxyReloadConfig: vi.fn(() => Promise.resolve(null)),
  proxySelectNode: vi.fn(() => Promise.resolve({ groups: [], trafficMode: "rule" })),
  proxySetTrafficMode: vi.fn(),
  proxyStartMonitor: vi.fn(() => Promise.resolve({ state: "running", running: true, stale: false, message: null })),
  proxyStopMonitor: vi.fn(() => Promise.resolve({ state: "stopped", running: false, stale: true, message: null })),
  proxyTestDelay: vi.fn(() => Promise.resolve([])),
  copyProfiles: vi.fn(),
  dedupeProfiles: vi.fn(),
  deleteSubscriptions: vi.fn(),
  deleteProfiles: vi.fn(),
  deleteRoutingRules: vi.fn(),
  deleteRoutings: vi.fn(),
  disconnectCore: vi.fn(),
  generateQrCode: vi.fn(() => Promise.resolve({ mimeType: "image/svg+xml", svg: "<svg />" })),
  getWindowChromeConfig: vi.fn(() => Promise.resolve({ titleBarLayout: "none" })),
  importConfigTemplate: vi.fn(() =>
    Promise.resolve({
      sources: {
        geoSourceUrl: null,
        srsSourceUrl: null,
        routeRulesTemplateSourceUrl: null,
      },
      routingIds: ["routing-default"],
      activeRoutingId: "routing-default",
      reusedExistingRouting: false,
    }),
  ),
  importProfilesFromText: vi.fn(),
  IpcCommandError: class IpcCommandError extends Error {},
  listGroupChildCandidates: vi.fn(() => Promise.resolve([])),
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
  listRoutings: vi.fn(() => Promise.resolve([])),
  listProfiles: vi.fn(() => Promise.resolve({ entries: [], undecodableProfiles: 0 })),
  listSubscriptionMetadata: vi.fn(() => Promise.resolve([])),
  listSubscriptions: vi.fn(() => Promise.resolve([])),
  loadAppSettings: vi.fn(() => new Promise(() => undefined)),
  loadUiPreferences: vi.fn(() => Promise.resolve({ language: "en", theme: "system" })),
  moveRoutingRule: vi.fn(),
  moveProfile: vi.fn(),
  previewGroupProfile: vi.fn(() =>
    Promise.resolve({
      validation: { childIndexIds: [], errors: [], normalizedChildItems: "", valid: true, warnings: [] },
      singboxRoutes: [],
    }),
  ),
  restartCore: vi.fn(),
  runtimeStatus: vi.fn(() =>
    Promise.resolve({
      activeProfileId: null,
      mainPid: null,
      prePid: null,
      activeTunBackend: null,
      runningCoreType: null,
      state: "disconnected",
    }),
  ),
  saveProfile: vi.fn(),
  saveGroupProfile: vi.fn(),
  saveRouting: vi.fn(),
  saveRoutingRule: vi.fn(),
  saveAppSettings: vi.fn(),
  saveConfigSources: vi.fn((settings) => Promise.resolve(settings)),
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
      pacAvailable: false,
      pacEnabled: false,
      processRulesEffective: false,
      vpnAvailable: true,
    }),
  ),
  setWindowAcrylic: vi.fn(() => Promise.resolve(null)),
  speedtestStatus: vi.fn(() => Promise.resolve({ running: false })),
  sortProfiles: vi.fn(),
  tunRequestElevation: vi.fn(),
  systemProxyStatus: vi.fn(() =>
    Promise.resolve({
      management: "automatic",
      observation: "unknown",
      manualCleanupRequired: false,
      effectiveMode: "forcedClear",
      exceptions: "",
      pacAvailable: false,
      pacUrl: null,
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
  updateGeoAssets: vi.fn(() => Promise.resolve([])),
  updateSrsAssets: vi.fn(() => Promise.resolve([])),
  updateSubscriptions: vi.fn(),
  useRuntimeEventStore: runtimeStoreMock.useRuntimeEventStore,
}));

function renderApp() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
    },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>,
  );
}

describe("App", () => {
  beforeEach(async () => {
    vi.useRealTimers();
    resetTestDom();
    runtimeStoreMock.reset();
    useShellStore.setState({
      activeTab: "profiles",
      connectionsView: "connections",
      navigationGuard: null,
      pendingTab: null,
    });
    useToastStore.setState({ toasts: [] });
    usePreferencesStore.setState({ themeMode: "system" });
    window.history.replaceState({}, "", "/");
    window.localStorage.clear();
    document.documentElement.className = "";
    vi.mocked(loadUiPreferences).mockReset();
    vi.mocked(loadUiPreferences).mockResolvedValue({ language: "en", theme: "system" });
    vi.mocked(proxyCloseConnection).mockClear();
    vi.mocked(proxyListConnections).mockClear();
    vi.mocked(proxyStartMonitor).mockClear();
    vi.mocked(proxyStopMonitor).mockClear();
    vi.mocked(proxyCloseConnection).mockResolvedValue({ connections: [], downloadTotal: 0, uploadTotal: 0 });
    vi.mocked(proxyListConnections).mockResolvedValue({ connections: [], downloadTotal: 0, uploadTotal: 0 });
    vi.mocked(proxyStartMonitor).mockResolvedValue({ state: "running", running: true, stale: false, message: null });
    vi.mocked(proxyStopMonitor).mockResolvedValue({ state: "stopped", running: false, stale: true, message: null });
    await changeLocale("en");
  });

  afterEach(() => {
    vi.useRealTimers();
    resetTestDom();
    delete (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  it("renders the six-item sidebar nav with the speed footer", () => {
    renderApp();

    const sidebar = screen.getByRole("complementary");
    const footer = screen.getByTestId("sidebar-footer");
    const tablist = screen.getByRole("tablist", { name: "Main sections" });

    const tabNames = within(tablist)
      .getAllByRole("tab")
      .map((tab) => tab.textContent);
    expect(tabNames).toEqual(["Home", "Proxies", "Profiles", "Settings", "Connections", "Rules"]);
    expect(footer).toHaveTextContent("Disconnected");
    expect(footer).toHaveTextContent("Up 0 B/s");
    expect(footer).toHaveTextContent("Down 0 B/s");
    expect(screen.queryByTestId("status-bar")).not.toBeInTheDocument();
    expect(within(sidebar).queryByRole("button", { name: "Settings" })).toBeNull();
    expect(within(sidebar).queryByRole("button", { name: "Theme" })).toBeNull();
  });

  it("defaults to the connection home hero", async () => {
    useShellStore.setState({ activeTab: "home" });

    renderApp();

    const hero = await screen.findByRole("region", { name: "Connection home" });
    // The app name is the Home page's h1 (the sidebar brand is a plain label).
    expect(within(hero).getByRole("heading", { level: 1, name: "VoyaVPN" })).toBeInTheDocument();
    expect(within(hero).getByRole("button", { name: "Connect" })).toBeInTheDocument();
    expect(within(hero).getByText("Not protected")).toBeInTheDocument();
    expect(screen.getByTestId("sidebar-footer")).toHaveTextContent("Disconnected");
  });

  it("falls back to a supported locale when the backend stores a removed language", async () => {
    await changeLocale("zh-Hans", { persist: false });
    vi.mocked(loadUiPreferences).mockResolvedValue({ language: "fa", theme: "system" });
    renderApp();

    await waitFor(() => expect(document.documentElement).toHaveAttribute("lang", "en"));
    expect(document.documentElement).toHaveAttribute("dir", "ltr");
    expect(screen.getByRole("tab", { name: "Profiles" })).toBeInTheDocument();
  });

  it("hydrates the theme through the dedicated preferences query", async () => {
    vi.mocked(loadUiPreferences).mockResolvedValue({ language: "en", theme: "dark" });

    renderApp();

    await waitFor(() => expect(document.documentElement).toHaveClass("dark"));
  });

  it("opens the in-shell settings screen from the sidebar tab", async () => {
    renderApp();

    await activateTab(/Settings/);

    expect(await screen.findByRole("region", { name: "Settings" })).toBeInTheDocument();
    expect(
      await screen.findByRole("tab", { name: "General", selected: true }),
    ).toBeInTheDocument();
    expect(screen.getByRole("complementary")).toBeInTheDocument();
  });

  it("shows Connections immediately and defers monitor plus query work", async () => {
    await import("@/features/proxy/connections-screen");
    vi.useFakeTimers();
    (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};

    renderApp();

    await activateTab(/Connections/);

    expect(screen.getByRole("heading", { level: 1, name: "Connections" })).toBeInTheDocument();
    expect(proxyStartMonitor).not.toHaveBeenCalled();
    expect(proxyListConnections).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(20);
    });
    expect(proxyListConnections).toHaveBeenCalledTimes(1);
    expect(proxyStartMonitor).not.toHaveBeenCalled();
    expect(runtimeStoreMock.getState().setProxyMonitorStarting).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(80);
    });
    expect(proxyStartMonitor).toHaveBeenCalledTimes(1);
    expect(runtimeStoreMock.getState().setProxyMonitorStarting).toHaveBeenCalledTimes(1);
    expect(runtimeStoreMock.getState().proxyMonitorStatus).toEqual({
      message: null,
      running: true,
      stale: false,
      state: "running",
    });
    expect(
      vi.mocked(runtimeStoreMock.getState().setProxyMonitorStarting).mock.invocationCallOrder[0]!,
    ).toBeLessThan(vi.mocked(proxyStartMonitor).mock.invocationCallOrder[0]!);

    await activateTab(/Profiles/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_999);
    });
    expect(proxyStopMonitor).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(proxyStopMonitor).toHaveBeenCalledTimes(1);
    expect(runtimeStoreMock.getState().proxyMonitorStatus).toEqual({
      message: null,
      running: false,
      stale: true,
      state: "stopped",
    });
  });

  it("keeps the monitor running during rapid switches between proxy runtime tabs", async () => {
    vi.useFakeTimers();
    (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};

    renderApp();

    await activateTab(/Proxies/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(50);
    });

    await activateTab(/Connections/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });

    expect(proxyStartMonitor).toHaveBeenCalledTimes(1);
    expect(proxyStopMonitor).not.toHaveBeenCalled();

    await activateTab(/Proxies/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000);
    });

    expect(proxyStartMonitor).toHaveBeenCalledTimes(1);
    expect(proxyStopMonitor).not.toHaveBeenCalled();
    expect(runtimeStoreMock.getState().proxyMonitorStatus).toEqual({
      message: null,
      running: true,
      stale: false,
      state: "running",
    });
  });

  it("keeps the proxy monitor running while viewing the logs sub-tab", async () => {
    vi.useFakeTimers();
    (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};

    renderApp();

    await activateTab(/Connections/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(proxyStartMonitor).toHaveBeenCalledTimes(1);

    const page = screen.getByRole("region", { name: "Connections" });
    const logsTab = within(page).getByRole("tab", { name: "Logs" });
    await act(async () => {
      fireEvent.mouseDown(logsTab);
      fireEvent.click(logsTab);
      await Promise.resolve();
    });

    expect(screen.getByText("No log lines")).toBeInTheDocument();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000);
    });
    expect(proxyStopMonitor).not.toHaveBeenCalled();
  });

  it("marks cached proxy monitor data failed and shows a toast when start fails", async () => {
    vi.useFakeTimers();
    (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    vi.mocked(proxyStartMonitor).mockRejectedValueOnce(new Error("start unavailable"));

    renderApp();

    await activateTab(/Proxies/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });

    expect(proxyStartMonitor).toHaveBeenCalledTimes(1);
    expect(runtimeStoreMock.getState().setProxyMonitorStarting).toHaveBeenCalledTimes(1);
    expect(runtimeStoreMock.getState().setProxyMonitorFailed).toHaveBeenCalledWith("start unavailable");
    expect(runtimeStoreMock.getState().proxyMonitorStatus).toEqual({
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
    (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    vi.mocked(proxyStopMonitor).mockRejectedValueOnce(new Error("stop unavailable"));

    renderApp();

    await activateTab(/Proxies/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(runtimeStoreMock.getState().proxyMonitorStatus.state).toBe("running");

    await activateTab(/Profiles/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000);
    });

    expect(proxyStopMonitor).toHaveBeenCalledTimes(1);
    expect(runtimeStoreMock.getState().setProxyMonitorFailed).toHaveBeenCalledWith("stop unavailable");
    expect(runtimeStoreMock.getState().proxyMonitorStatus).toEqual({
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

  it("shows stale monitor status in Proxies without replacing toolbar controls", async () => {
    const user = userEvent.setup();
    runtimeStoreMock.getState().setProxyMonitorStopped();

    renderApp();

    await user.click(mainNavTab(/Proxies/));

    expect(screen.getByRole("status", { name: "Stale: Stopped" })).toBeInTheDocument();
    // Scope toolbar-control assertions to the Proxies region. The home
    // hero's system-proxy selector also exposes "Direct"/"Global" buttons, so
    // scoping keeps these queries unambiguous and robust to shell layout.
    const proxies = screen.getByRole("region", { name: "Proxies" });
    expect(within(proxies).queryByText(/Up .*\/s/)).not.toBeInTheDocument();
    expect(within(proxies).queryByText(/Down .*\/s/)).not.toBeInTheDocument();
    expect(within(proxies).getByRole("button", { name: "Rule" })).toBeInTheDocument();
    expect(within(proxies).getByRole("button", { name: "Global" })).toBeInTheDocument();
    expect(within(proxies).getByRole("button", { name: "Direct" })).toBeInTheDocument();
    expect(within(proxies).getByRole("button", { name: "Reload core configuration" })).toBeInTheDocument();
    expect(within(proxies).getByRole("button", { name: "Test all" })).toBeInTheDocument();
    expect(within(proxies).getByRole("button", { name: "Refresh runtime state" })).toBeInTheDocument();
  });

  it("shows failed monitor status with its message in Connections while keeping data controls visible", async () => {
    const user = userEvent.setup();
    const message = "monitor stream failed after retry budget was exhausted";
    runtimeStoreMock.getState().setProxyMonitorFailed(message);
    vi.mocked(proxyListConnections).mockResolvedValue({
      connections: [makeConnection(0, { host: "alpha.example:443", id: "alpha" })],
      downloadTotal: 4096,
      uploadTotal: 1024,
    });

    renderApp();

    await user.click(mainNavTab(/Connections/));
    await waitFor(() => expect(screen.getByText("alpha.example:443")).toBeInTheDocument());

    expect(screen.getByRole("status", { name: `Failed: ${message}` })).toBeInTheDocument();
    expect(screen.getByText(message)).toBeInTheDocument();
    expect(screen.getByText("Cumulative upload 1.0 KB")).toBeInTheDocument();
    expect(screen.getByText("Cumulative download 4.0 KB")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Filter connections" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close all" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeInTheDocument();
  });

  it("clears the selected connection when it leaves and re-enters the filtered snapshot", async () => {
    const user = userEvent.setup();
    vi.mocked(proxyListConnections).mockResolvedValue({
      connections: [
        makeConnection(0, { host: "alpha.example:443", id: "alpha" }),
        makeConnection(1, { host: "beta.example:443", id: "beta" }),
      ],
      downloadTotal: 2,
      uploadTotal: 1,
    });

    renderApp();

    await user.click(mainNavTab(/Connections/));
    await waitFor(() => expect(screen.getByText("alpha.example:443")).toBeInTheDocument());

    await user.click(screen.getByText("alpha.example:443"));
    await waitFor(() => expect(screen.getByRole("button", { name: "Close" })).toBeEnabled());

    const filterInput = screen.getByRole("textbox", { name: "Filter connections" });
    await user.type(filterInput, "beta");
    await waitFor(() => expect(screen.queryByText("alpha.example:443")).not.toBeInTheDocument());
    await waitFor(() => expect(screen.getByRole("button", { name: "Close" })).toBeDisabled());

    await user.clear(filterInput);
    await waitFor(() => expect(screen.getByText("alpha.example:443")).toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Close" })).toBeDisabled();
  });

  it("manual refresh seeds Connections snapshots without clearing stale monitor status", async () => {
    const user = userEvent.setup();
    const cachedSnapshot = {
      connections: [makeConnection(0, { host: "cached.example:443", id: "cached" })],
      downloadTotal: 100,
      uploadTotal: 50,
    };
    const refreshedSnapshot = {
      connections: [makeConnection(1, { host: "fresh.example:443", id: "fresh" })],
      downloadTotal: 4096,
      uploadTotal: 1024,
    };
    runtimeStoreMock.getState().setProxyMonitorFailed("monitor offline");
    runtimeStoreMock.getState().setProxyConnections(cachedSnapshot);
    vi.mocked(runtimeStoreMock.getState().setProxyConnections).mockClear();
    vi.mocked(proxyListConnections)
      .mockResolvedValueOnce(cachedSnapshot)
      .mockResolvedValueOnce(refreshedSnapshot);

    renderApp();

    await user.click(mainNavTab(/Connections/));
    await waitFor(() => expect(proxyListConnections).toHaveBeenCalledTimes(1));
    expect(screen.getByText("cached.example:443")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Refresh" }));

    await waitFor(() => expect(proxyListConnections).toHaveBeenCalledTimes(2));
    await waitFor(() =>
      expect(runtimeStoreMock.getState().setProxyConnections).toHaveBeenCalledWith(refreshedSnapshot),
    );
    await waitFor(() => expect(screen.getByText("fresh.example:443")).toBeInTheDocument());
    expect(runtimeStoreMock.getState().proxyMonitorStatus).toEqual({
      message: "monitor offline",
      running: false,
      stale: true,
      state: "failed",
    });
    expect(screen.getByRole("status", { name: "Failed: monitor offline" })).toBeInTheDocument();
  });

  it("close selected and close all update snapshots without clearing stale monitor status", async () => {
    const user = userEvent.setup();
    const initialSnapshot = {
      connections: [
        makeConnection(0, { host: "alpha.example:443", id: "alpha" }),
        makeConnection(1, { host: "beta.example:443", id: "beta" }),
      ],
      downloadTotal: 2,
      uploadTotal: 1,
    };
    const selectedClosedSnapshot = {
      connections: [makeConnection(1, { host: "beta.example:443", id: "beta" })],
      downloadTotal: 1,
      uploadTotal: 1,
    };
    const allClosedSnapshot = { connections: [], downloadTotal: 0, uploadTotal: 0 };
    runtimeStoreMock.getState().setProxyMonitorFailed("monitor offline");
    vi.mocked(proxyListConnections).mockResolvedValue(initialSnapshot);
    vi.mocked(proxyCloseConnection)
      .mockResolvedValueOnce(selectedClosedSnapshot)
      .mockResolvedValueOnce(allClosedSnapshot);

    renderApp();

    await user.click(mainNavTab(/Connections/));
    await waitFor(() => expect(screen.getByText("alpha.example:443")).toBeInTheDocument());

    await user.click(screen.getByText("alpha.example:443"));
    await waitFor(() => expect(screen.getByRole("button", { name: "Close" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Close" }));

    await waitFor(() => expect(vi.mocked(proxyCloseConnection).mock.calls.at(0)?.[0]).toBe("alpha"));
    await waitFor(() => expect(screen.queryByText("alpha.example:443")).not.toBeInTheDocument());
    expect(screen.getByText("beta.example:443")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("button", { name: "Close" })).toBeDisabled());
    expect(runtimeStoreMock.getState().proxyMonitorStatus).toEqual({
      message: "monitor offline",
      running: false,
      stale: true,
      state: "failed",
    });
    expect(screen.getByRole("status", { name: "Failed: monitor offline" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Close all" }));

    await waitFor(() => expect(vi.mocked(proxyCloseConnection).mock.calls.at(1)?.[0]).toBeNull());
    await waitFor(() => expect(screen.getByText("No connections")).toBeInTheDocument());
    expect(runtimeStoreMock.getState().proxyMonitorStatus).toEqual({
      message: "monitor offline",
      running: false,
      stale: true,
      state: "failed",
    });
    expect(screen.getByRole("status", { name: "Failed: monitor offline" })).toBeInTheDocument();
  });

  it("virtualizes large Connections result sets across stale and live monitor states", async () => {
    const user = userEvent.setup();
    runtimeStoreMock.getState().setProxyMonitorFailed("monitor offline");
    vi.mocked(proxyListConnections).mockResolvedValue({
      connections: makeConnections(200),
      downloadTotal: 200,
      uploadTotal: 100,
    });

    renderApp();

    await user.click(mainNavTab(/Connections/));

    await waitFor(() => expect(screen.getByText("bulk-0.example:443")).toBeInTheDocument());
    expect(screen.queryAllByText(/bulk-\d+\.example:443/).length).toBeLessThan(80);
    expect(screen.getByRole("status", { name: "Failed: monitor offline" })).toBeInTheDocument();

    runtimeStoreMock.getState().setProxyMonitorRunning();
    await user.click(screen.getByRole("button", { name: "Refresh" }));

    await waitFor(() => expect(screen.getByRole("status", { name: "Live" })).toBeInTheDocument());
    expect(screen.queryAllByText(/bulk-\d+\.example:443/).length).toBeLessThan(80);
  });
});

function mainNavTab(name: RegExp) {
  return within(screen.getByRole("tablist", { name: "Main sections" })).getByRole("tab", { name });
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

function makeConnection(index: number, overrides: Partial<ProxyConnectionItem> = {}): ProxyConnectionItem {
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
