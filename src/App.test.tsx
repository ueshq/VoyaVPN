import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, vi } from "vitest";

import { App } from "./App";
import { fontToCss } from "./config/fonts";
import { changeLocale } from "./i18n";
import {
  clashCloseConnection,
  clashListConnections,
  clashStartMonitor,
  clashStopMonitor,
  loadAppConfig,
  saveAppConfig,
} from "@/ipc";
import type {
  AppConfig_Serialize,
  ClashConnectionItem,
  ClashConnectionsSnapshot,
  ClashTrafficEvent,
  UiItem_Serialize,
} from "@/ipc/bindings";
import { DEFAULT_FONT, DEFAULT_FONT_SIZE, usePreferencesStore } from "@/stores/preferences-store";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn(() => Promise.resolve("0.1.0")),
}));
vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: vi.fn(() => Promise.resolve()),
}));
vi.mock("@tauri-apps/plugin-updater", () => ({
  check: vi.fn(() => Promise.resolve(null)),
}));

type TestClashMonitorState = "starting" | "running" | "stopped" | "failed";

type TestClashMonitorStatus = {
  message: string | null;
  running: boolean;
  stale: boolean;
  state: TestClashMonitorState;
};

type TestRuntimeEventState = {
  clearLogs: () => void;
  clashConnections: ClashConnectionsSnapshot | null;
  clashMonitorStatus: TestClashMonitorStatus;
  clashTraffic: ClashTrafficEvent | null;
  coreState: null;
  lastTransientEvent: null;
  logLines: never[];
  pushTransientEvent: () => void;
  serverStatsByProfileId: Record<string, never>;
  setClashConnections: (snapshot: ClashConnectionsSnapshot) => void;
  setClashMonitorFailed: (message?: string | null) => void;
  setClashMonitorRunning: (message?: string | null) => void;
  setClashMonitorStarting: (message?: string | null) => void;
  setClashMonitorStatus: (status: TestClashMonitorStatus) => void;
  setClashMonitorStopped: (message?: string | null) => void;
  setClashTraffic: (event: ClashTrafficEvent) => void;
  setCoreState: () => void;
  setSysProxy: () => void;
  setTun: () => void;
  speedtestResultsByProfileId: Record<string, never>;
  statistics: null;
  sysProxy: null;
  tun: null;
};

type TestRuntimeEventStore = {
  getState: () => TestRuntimeEventState;
  reset: () => void;
  useRuntimeEventStore: {
    (selector: (state: TestRuntimeEventState) => unknown): unknown;
    getState: () => TestRuntimeEventState;
  };
};

const runtimeStoreMock = vi.hoisted<TestRuntimeEventStore>(() => {
  const initialMonitorStatus: TestClashMonitorStatus = {
    message: null,
    running: false,
    stale: true,
    state: "stopped",
  };
  let state: TestRuntimeEventState;

  function makeMonitorStatus(
    monitorState: TestClashMonitorState,
    running: boolean,
    stale: boolean,
    message: string | null,
  ): TestClashMonitorStatus {
    return { message, running, stale, state: monitorState };
  }

  function makeState(): TestRuntimeEventState {
    const nextState = {
      clearLogs: vi.fn(),
      clashConnections: null,
      clashMonitorStatus: initialMonitorStatus,
      clashTraffic: null,
      coreState: null,
      lastTransientEvent: null,
      logLines: [],
      pushTransientEvent: vi.fn(),
      serverStatsByProfileId: {},
      setClashConnections: vi.fn((snapshot: ClashConnectionsSnapshot) => {
        state.clashConnections = snapshot;
      }),
      setClashMonitorFailed: vi.fn((message: string | null = null) => {
        state.clashMonitorStatus = makeMonitorStatus("failed", false, true, message);
      }),
      setClashMonitorRunning: vi.fn((message: string | null = null) => {
        state.clashMonitorStatus = makeMonitorStatus("running", true, false, message);
      }),
      setClashMonitorStarting: vi.fn((message: string | null = null) => {
        state.clashMonitorStatus = makeMonitorStatus(
          "starting",
          false,
          state.clashMonitorStatus.stale,
          message,
        );
      }),
      setClashMonitorStatus: vi.fn((status: TestClashMonitorStatus) => {
        state.clashMonitorStatus = status;
      }),
      setClashMonitorStopped: vi.fn((message: string | null = null) => {
        state.clashMonitorStatus = makeMonitorStatus("stopped", false, true, message);
      }),
      setClashTraffic: vi.fn((event: ClashTrafficEvent) => {
        state.clashTraffic = event;
      }),
      setCoreState: vi.fn(),
      setSysProxy: vi.fn(),
      setTun: vi.fn(),
      speedtestResultsByProfileId: {},
      statistics: null,
      sysProxy: null,
      tun: null,
    } satisfies TestRuntimeEventState;

    return nextState;
  }

  state = makeState();

  const useRuntimeEventStore = Object.assign(
    vi.fn((selector: (state: TestRuntimeEventState) => unknown) => selector(state)),
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
  autostartStatus: vi.fn(() =>
    Promise.resolve({
      artifactKind: "linuxDesktopFile",
      artifactName: "VoyaVPN.desktop",
      artifactPath: "/home/test/.config/autostart/VoyaVPN.desktop",
      enabled: false,
      platform: "linux",
    }),
  ),
  clashCloseConnection: vi.fn(() => Promise.resolve({ connections: [], downloadTotal: 0, uploadTotal: 0 })),
  clashListConnections: vi.fn(() => Promise.resolve({ connections: [], downloadTotal: 0, uploadTotal: 0 })),
  clashListProxies: vi.fn(() => Promise.resolve({ allNodes: [], groups: [], ruleMode: 0 })),
  clashReloadConfig: vi.fn(() => Promise.resolve(null)),
  clashSelectProxy: vi.fn(() => Promise.resolve({ allNodes: [], groups: [], ruleMode: 0 })),
  clashSetRuleMode: vi.fn(),
  clashStartMonitor: vi.fn(() => Promise.resolve({ state: "running", running: true, stale: false, message: null })),
  clashStopMonitor: vi.fn(() => Promise.resolve({ state: "stopped", running: false, stale: true, message: null })),
  clashTestDelay: vi.fn(() => Promise.resolve([])),
  copyProfiles: vi.fn(),
  dedupeProfiles: vi.fn(),
  deleteSubscriptions: vi.fn(),
  deleteProfiles: vi.fn(),
  deleteRoutingRules: vi.fn(),
  deleteRoutings: vi.fn(),
  diagnosticsStatus: vi.fn(() =>
    Promise.resolve({
      deliveryConfigured: false,
      enabled: true,
      queuedBytes: 0,
      queuedEvents: 0,
    }),
  ),
  disconnectCore: vi.fn(),
  generateQrCode: vi.fn(() => Promise.resolve({ mimeType: "image/svg+xml", svg: "<svg />" })),
  getWindowChromeConfig: vi.fn(() => Promise.resolve({ titleBarLayout: "none" })),
  globalHotkeyStatus: vi.fn(() =>
    Promise.resolve({
      actions: [
        { action: 0, label: "Show window" },
        { action: 1, label: "Clear system proxy" },
        { action: 2, label: "Set system proxy" },
        { action: 3, label: "Leave system proxy unchanged" },
        { action: 4, label: "Set PAC proxy" },
      ],
      registered: [],
      settings: [
        { Alt: false, Control: false, EGlobalHotkey: 0, KeyCode: null, Shift: false },
        { Alt: false, Control: false, EGlobalHotkey: 1, KeyCode: null, Shift: false },
        { Alt: false, Control: false, EGlobalHotkey: 2, KeyCode: null, Shift: false },
        { Alt: false, Control: false, EGlobalHotkey: 3, KeyCode: null, Shift: false },
        { Alt: false, Control: false, EGlobalHotkey: 4, KeyCode: null, Shift: false },
      ],
    }),
  ),
  importRoutingTemplates: vi.fn(),
  importProfilesFromText: vi.fn(),
  IpcCommandError: class IpcCommandError extends Error {},
  listGroupChildCandidates: vi.fn(() => Promise.resolve([])),
  loadDnsSettings: vi.fn(() =>
    Promise.resolve({
      simpleDnsItem: {},
      singboxDnsItem: {
        Id: "dns-singbox",
        Remarks: "sing-box",
        Enabled: false,
        UseSystemHosts: false,
      },
      defaults: {
        singboxNormalDns: "{\"servers\":[]}",
        singboxTunDns: "{\"servers\":[]}",
      },
    }),
  ),
  loadRulesetGeoSources: vi.fn(() => Promise.resolve({ geoSourceUrl: null, srsSourceUrl: null })),
  listRoutings: vi.fn(() => Promise.resolve([])),
  listProfiles: vi.fn(() => Promise.resolve([])),
  listSubscriptions: vi.fn(() => Promise.resolve([])),
  loadAppConfig: vi.fn(() =>
    Promise.resolve({
      ConstItem: {
        RouteRulesTemplateSourceUrl: null,
      },
      UIItem: {
        ColorPrimaryName: "Teal",
        CurrentFontFamily: "",
        CurrentFontSize: 16,
        CurrentLanguage: "en",
        CurrentTheme: "FollowSystem",
      },
    }),
  ),
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
      runningCoreType: null,
      state: "disconnected",
    }),
  ),
  saveProfile: vi.fn(),
  saveGroupProfile: vi.fn(),
  saveGlobalHotkeys: vi.fn((settings) => Promise.resolve({ actions: [], registered: [], settings })),
  saveRouting: vi.fn(),
  saveRoutingRule: vi.fn(),
  saveRulesetGeoSources: vi.fn((settings) => Promise.resolve(settings)),
  saveAppConfig: vi.fn((config) => Promise.resolve(config)),
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
  setDiagnosticsEnabled: vi.fn((enabled) =>
    Promise.resolve({
      deliveryConfigured: false,
      enabled,
      queuedBytes: 0,
      queuedEvents: 0,
    }),
  ),
  setSystemProxyMode: vi.fn(() =>
    Promise.resolve({
      effectiveMode: 0,
      exceptions: "",
      pacAvailable: false,
      pacUrl: null,
      proxy: null,
      requestedMode: 0,
    }),
  ),
  setWindowAcrylic: vi.fn(() => Promise.resolve(null)),
  setTunEnabled: vi.fn(() =>
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
      providerState: "notApplicable",
      requiresElevation: false,
      restoreOnDisconnect: true,
    }),
  ),
  speedtestStatus: vi.fn(() => Promise.resolve({ running: false })),
  sortProfiles: vi.fn(),
  validateGroupProfile: vi.fn(() =>
    Promise.resolve({ childIndexIds: [], errors: [], normalizedChildItems: "", valid: true, warnings: [] }),
  ),
  tunRequestElevation: vi.fn(),
  tunRevokeElevation: vi.fn(),
  systemProxyStatus: vi.fn(() =>
    Promise.resolve({
      effectiveMode: 0,
      exceptions: "",
      pacAvailable: false,
      pacUrl: null,
      proxy: null,
      requestedMode: 0,
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
      providerState: "notApplicable",
      requiresElevation: false,
      restoreOnDisconnect: true,
    }),
  ),
  checkUpdates: vi.fn(() => Promise.resolve({ preRelease: false, results: [], targets: [] })),
  downloadUpdates: vi.fn(() => Promise.resolve({ preRelease: false, results: [], targets: [] })),
  manualAppUpdateLinks: vi.fn(() =>
    Promise.resolve({
      arch: "x64",
      channel: "stable",
      currentVersion: "0.1.0",
      downloads: [],
      hasUpdate: false,
      releaseIndexUrl: "https://cdn.voyavpn.test/stable/release-index.json",
      remoteVersion: null,
      target: "linux",
    }),
  ),
  recordAppUpdateDiagnostic: vi.fn(() => Promise.resolve()),
  saveUpdatePreferences: vi.fn(() => Promise.resolve({ preRelease: false, targets: [] })),
  updateStatus: vi.fn(() =>
    Promise.resolve({
      preRelease: false,
      targets: [
        {
          acquisition: "appPackage",
          coreType: null,
          id: "app",
          kind: "app",
          license: "MIT",
          name: "VoyaVPN",
          redistributeInInstaller: true,
          remarks: "application package update",
          selected: true,
          updateSupported: true,
        },
      ],
    }),
  ),
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
    useShellStore.setState({ activeTab: "profiles" });
    useToastStore.setState({ toasts: [] });
    // Reset the persisted preferences singleton so each test re-hydrates from
    // its own loadAppConfig mock; otherwise a prior test leaves appConfigLoaded
    // true and the theme/font hydration effect short-circuits.
    usePreferencesStore.setState({
      appConfigLoaded: false,
      font: DEFAULT_FONT,
      fontSize: DEFAULT_FONT_SIZE,
      themeMode: "system",
    });
    window.localStorage.clear();
    document.documentElement.className = "";
    document.documentElement.style.removeProperty("--app-font-family");
    document.documentElement.style.removeProperty("--app-font-size");
    vi.mocked(loadAppConfig).mockClear();
    vi.mocked(saveAppConfig).mockClear();
    vi.mocked(clashCloseConnection).mockClear();
    vi.mocked(clashListConnections).mockClear();
    vi.mocked(clashStartMonitor).mockClear();
    vi.mocked(clashStopMonitor).mockClear();
    vi.mocked(clashCloseConnection).mockResolvedValue({ connections: [], downloadTotal: 0, uploadTotal: 0 });
    vi.mocked(clashListConnections).mockResolvedValue({ connections: [], downloadTotal: 0, uploadTotal: 0 });
    vi.mocked(clashStartMonitor).mockResolvedValue({ state: "running", running: true, stale: false, message: null });
    vi.mocked(clashStopMonitor).mockResolvedValue({ state: "stopped", running: false, stale: true, message: null });
    vi.mocked(loadAppConfig).mockResolvedValue(makeAppConfig());
    vi.mocked(saveAppConfig).mockImplementation(async (config) => config as AppConfig_Serialize);
    await changeLocale("en");
  });

  afterEach(() => {
    vi.useRealTimers();
    resetTestDom();
    delete (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  });

  it("renders the app shell tabs and status bar", () => {
    renderApp();

    expect(screen.getByRole("heading", { name: "VoyaVPN" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Home/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Profiles/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Routing/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /DNS/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Clash Proxies/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Clash Connections/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Logs/ })).toBeInTheDocument();
    expect(screen.getByTestId("status-bar")).toHaveTextContent("Disconnected");
    expect(screen.getByTestId("status-bar")).toHaveTextContent("Route: /profiles");
  });

  it("defaults to the connection home hero", () => {
    useShellStore.setState({ activeTab: "home" });

    renderApp();

    const hero = screen.getByRole("region", { name: "Connection home" });
    expect(within(hero).getByRole("button", { name: "Connect" })).toBeInTheDocument();
    expect(within(hero).getByText("Not protected")).toBeInTheDocument();
    expect(screen.getByTestId("status-bar")).toHaveTextContent("Disconnected");
    expect(screen.getByTestId("status-bar")).toHaveTextContent("Route: /home");
  });

  it("switches document direction through the RTL locale", async () => {
    const user = userEvent.setup();

    renderApp();

    // Language selection now lives in the Settings modal rather than a header
    // toggle, so reach the RTL locale through the sidebar footer's Settings entry.
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.click(screen.getByRole("button", { name: "FA" }));

    await waitFor(() => expect(document.documentElement).toHaveAttribute("dir", "rtl"));

    // The modal traps focus and hides the rest of the tree, so close it before
    // asserting the now-Farsi sidebar nav.
    await user.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    expect(screen.getByRole("tab", { name: /نمایه/ })).toBeInTheDocument();
  });

  it("hydrates and persists theme and strict font settings through app config", async () => {
    const user = userEvent.setup();
    vi.mocked(loadAppConfig).mockResolvedValue(
      makeAppConfig({
        UIItem: makeUiItem({
          ColorPrimaryName: "Rose",
          CurrentFontFamily: "Manrope",
          CurrentFontSize: 18,
          CurrentTheme: "Dark",
        }),
      }),
    );

    renderApp();

    await waitFor(() => expect(document.documentElement).toHaveClass("dark"));
    expect(document.documentElement).toHaveClass("font-manrope");
    expect(document.documentElement.style.getPropertyValue("--app-font-family")).toBe(fontToCss("manrope"));
    expect(document.documentElement.style.getPropertyValue("--app-font-size")).toBe("18px");

    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.click(screen.getByRole("button", { name: "Light" }));
    await user.click(screen.getByRole("button", { name: "Inter" }));

    await waitFor(() => {
      const savedConfig = vi.mocked(saveAppConfig).mock.calls.at(-1)?.[0];

      expect(savedConfig?.UIItem).toMatchObject({
        CurrentFontFamily: "Inter",
        CurrentFontSize: 18,
        CurrentTheme: "Light",
      });
      expect(savedConfig?.UIItem).not.toHaveProperty("ColorPrimaryName");
    });

    await user.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("shows Clash Connections immediately and defers monitor plus query work", async () => {
    vi.useFakeTimers();
    (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};

    renderApp();

    await activateTab(/Clash Connections/);

    expect(screen.getByRole("heading", { name: "Clash Connections" })).toBeInTheDocument();
    expect(screen.getByTestId("status-bar")).toHaveTextContent("Route: /clash/connections");
    expect(clashStartMonitor).not.toHaveBeenCalled();
    expect(clashListConnections).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(20);
    });
    expect(clashListConnections).toHaveBeenCalledTimes(1);
    expect(clashStartMonitor).not.toHaveBeenCalled();
    expect(runtimeStoreMock.getState().setClashMonitorStarting).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(80);
    });
    expect(clashStartMonitor).toHaveBeenCalledTimes(1);
    expect(runtimeStoreMock.getState().setClashMonitorStarting).toHaveBeenCalledTimes(1);
    expect(runtimeStoreMock.getState().clashMonitorStatus).toEqual({
      message: null,
      running: true,
      stale: false,
      state: "running",
    });
    expect(
      vi.mocked(runtimeStoreMock.getState().setClashMonitorStarting).mock.invocationCallOrder[0]!,
    ).toBeLessThan(vi.mocked(clashStartMonitor).mock.invocationCallOrder[0]!);

    await activateTab(/Profiles/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_999);
    });
    expect(clashStopMonitor).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1);
    });
    expect(clashStopMonitor).toHaveBeenCalledTimes(1);
    expect(runtimeStoreMock.getState().clashMonitorStatus).toEqual({
      message: null,
      running: false,
      stale: true,
      state: "stopped",
    });
  });

  it("keeps the monitor running during rapid switches between Clash tabs", async () => {
    vi.useFakeTimers();
    (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};

    renderApp();

    await activateTab(/Clash Proxies/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(50);
    });

    await activateTab(/Clash Connections/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });

    expect(clashStartMonitor).toHaveBeenCalledTimes(1);
    expect(clashStopMonitor).not.toHaveBeenCalled();

    await activateTab(/Clash Proxies/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000);
    });

    expect(clashStartMonitor).toHaveBeenCalledTimes(1);
    expect(clashStopMonitor).not.toHaveBeenCalled();
    expect(runtimeStoreMock.getState().clashMonitorStatus).toEqual({
      message: null,
      running: true,
      stale: false,
      state: "running",
    });
  });

  it("marks cached Clash monitor data failed and shows a toast when start fails", async () => {
    vi.useFakeTimers();
    (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    vi.mocked(clashStartMonitor).mockRejectedValueOnce(new Error("start unavailable"));

    renderApp();

    await activateTab(/Clash Proxies/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });

    expect(clashStartMonitor).toHaveBeenCalledTimes(1);
    expect(runtimeStoreMock.getState().setClashMonitorStarting).toHaveBeenCalledTimes(1);
    expect(runtimeStoreMock.getState().setClashMonitorFailed).toHaveBeenCalledWith("start unavailable");
    expect(runtimeStoreMock.getState().clashMonitorStatus).toEqual({
      message: "start unavailable",
      running: false,
      stale: true,
      state: "failed",
    });
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "start unavailable",
      title: "Clash",
    });
  });

  it("marks cached Clash monitor data failed and shows a toast when delayed stop fails", async () => {
    vi.useFakeTimers();
    (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    vi.mocked(clashStopMonitor).mockRejectedValueOnce(new Error("stop unavailable"));

    renderApp();

    await activateTab(/Clash Proxies/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(runtimeStoreMock.getState().clashMonitorStatus.state).toBe("running");

    await activateTab(/Profiles/);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000);
    });

    expect(clashStopMonitor).toHaveBeenCalledTimes(1);
    expect(runtimeStoreMock.getState().setClashMonitorFailed).toHaveBeenCalledWith("stop unavailable");
    expect(runtimeStoreMock.getState().clashMonitorStatus).toEqual({
      message: "stop unavailable",
      running: false,
      stale: true,
      state: "failed",
    });
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "stop unavailable",
      title: "Clash",
    });
  });

  it("shows stale monitor status in Clash Proxies without replacing toolbar controls", async () => {
    const user = userEvent.setup();
    runtimeStoreMock.getState().setClashMonitorStopped();
    runtimeStoreMock.getState().setClashTraffic({ down: 2048, up: 512 });

    renderApp();

    await user.click(screen.getByRole("tab", { name: /Clash Proxies/ }));

    expect(screen.getByRole("status", { name: "Stale: Stopped" })).toBeInTheDocument();
    expect(screen.getByText("Up 512 B/s")).toBeInTheDocument();
    expect(screen.getByText("Down 2.0 KB/s")).toBeInTheDocument();
    // Scope toolbar-control assertions to the Clash Proxies region. The home
    // hero's system-proxy selector also exposes "Direct"/"Global" buttons, so
    // scoping keeps these queries unambiguous and robust to shell layout.
    const proxies = screen.getByRole("region", { name: "Clash Proxies" });
    expect(within(proxies).getByRole("button", { name: "Rule" })).toBeInTheDocument();
    expect(within(proxies).getByRole("button", { name: "Global" })).toBeInTheDocument();
    expect(within(proxies).getByRole("button", { name: "Direct" })).toBeInTheDocument();
    expect(within(proxies).getByRole("button", { name: "Reload" })).toBeInTheDocument();
    expect(within(proxies).getByRole("button", { name: "Delay test" })).toBeInTheDocument();
    expect(within(proxies).getByRole("button", { name: "Refresh" })).toBeInTheDocument();
  });

  it("shows failed monitor status with its message in Clash Connections while keeping data controls visible", async () => {
    const user = userEvent.setup();
    const message = "monitor stream failed after retry budget was exhausted";
    runtimeStoreMock.getState().setClashMonitorFailed(message);
    vi.mocked(clashListConnections).mockResolvedValue({
      connections: [makeConnection(0, { host: "alpha.example:443", id: "alpha" })],
      downloadTotal: 4096,
      uploadTotal: 1024,
    });

    renderApp();

    await user.click(screen.getByRole("tab", { name: /Clash Connections/ }));
    await waitFor(() => expect(screen.getByText("alpha.example:443")).toBeInTheDocument());

    expect(screen.getByRole("status", { name: `Failed: ${message}` })).toBeInTheDocument();
    expect(screen.getByText(message)).toBeInTheDocument();
    expect(screen.getByText("Up 1.0 KB")).toBeInTheDocument();
    expect(screen.getByText("Down 4.0 KB")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Filter connections" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close all" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh" })).toBeInTheDocument();
  });

  it("clears selected Clash connection when it leaves and re-enters the filtered snapshot", async () => {
    const user = userEvent.setup();
    vi.mocked(clashListConnections).mockResolvedValue({
      connections: [
        makeConnection(0, { host: "alpha.example:443", id: "alpha" }),
        makeConnection(1, { host: "beta.example:443", id: "beta" }),
      ],
      downloadTotal: 2,
      uploadTotal: 1,
    });

    renderApp();

    await user.click(screen.getByRole("tab", { name: /Clash Connections/ }));
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

  it("manual refresh seeds Clash Connections snapshots without clearing stale monitor status", async () => {
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
    runtimeStoreMock.getState().setClashMonitorFailed("monitor offline");
    runtimeStoreMock.getState().setClashConnections(cachedSnapshot);
    vi.mocked(runtimeStoreMock.getState().setClashConnections).mockClear();
    vi.mocked(clashListConnections)
      .mockResolvedValueOnce(cachedSnapshot)
      .mockResolvedValueOnce(refreshedSnapshot);

    renderApp();

    await user.click(screen.getByRole("tab", { name: /Clash Connections/ }));
    await waitFor(() => expect(clashListConnections).toHaveBeenCalledTimes(1));
    expect(screen.getByText("cached.example:443")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Refresh" }));

    await waitFor(() => expect(clashListConnections).toHaveBeenCalledTimes(2));
    await waitFor(() =>
      expect(runtimeStoreMock.getState().setClashConnections).toHaveBeenCalledWith(refreshedSnapshot),
    );
    await waitFor(() => expect(screen.getByText("fresh.example:443")).toBeInTheDocument());
    expect(runtimeStoreMock.getState().clashMonitorStatus).toEqual({
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
    runtimeStoreMock.getState().setClashMonitorFailed("monitor offline");
    vi.mocked(clashListConnections).mockResolvedValue(initialSnapshot);
    vi.mocked(clashCloseConnection)
      .mockResolvedValueOnce(selectedClosedSnapshot)
      .mockResolvedValueOnce(allClosedSnapshot);

    renderApp();

    await user.click(screen.getByRole("tab", { name: /Clash Connections/ }));
    await waitFor(() => expect(screen.getByText("alpha.example:443")).toBeInTheDocument());

    await user.click(screen.getByText("alpha.example:443"));
    await waitFor(() => expect(screen.getByRole("button", { name: "Close" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Close" }));

    await waitFor(() => expect(vi.mocked(clashCloseConnection).mock.calls.at(0)?.[0]).toBe("alpha"));
    await waitFor(() => expect(screen.queryByText("alpha.example:443")).not.toBeInTheDocument());
    expect(screen.getByText("beta.example:443")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("button", { name: "Close" })).toBeDisabled());
    expect(runtimeStoreMock.getState().clashMonitorStatus).toEqual({
      message: "monitor offline",
      running: false,
      stale: true,
      state: "failed",
    });
    expect(screen.getByRole("status", { name: "Failed: monitor offline" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Close all" }));

    await waitFor(() => expect(vi.mocked(clashCloseConnection).mock.calls.at(1)?.[0]).toBeNull());
    await waitFor(() => expect(screen.getByText("No Clash connections")).toBeInTheDocument());
    expect(runtimeStoreMock.getState().clashMonitorStatus).toEqual({
      message: "monitor offline",
      running: false,
      stale: true,
      state: "failed",
    });
    expect(screen.getByRole("status", { name: "Failed: monitor offline" })).toBeInTheDocument();
  });

  it("virtualizes large Clash Connections result sets across stale and live monitor states", async () => {
    const user = userEvent.setup();
    runtimeStoreMock.getState().setClashMonitorFailed("monitor offline");
    vi.mocked(clashListConnections).mockResolvedValue({
      connections: makeConnections(200),
      downloadTotal: 200,
      uploadTotal: 100,
    });

    renderApp();

    await user.click(screen.getByRole("tab", { name: /Clash Connections/ }));

    await waitFor(() => expect(screen.getByText("bulk-0.example:443")).toBeInTheDocument());
    expect(screen.queryAllByText(/bulk-\d+\.example:443/).length).toBeLessThan(80);
    expect(screen.getByRole("status", { name: "Failed: monitor offline" })).toBeInTheDocument();

    runtimeStoreMock.getState().setClashMonitorRunning();
    await user.click(screen.getByRole("button", { name: "Refresh" }));

    await waitFor(() => expect(screen.getByRole("status", { name: "Live" })).toBeInTheDocument());
    expect(screen.queryAllByText(/bulk-\d+\.example:443/).length).toBeLessThan(80);
  });
});

async function activateTab(name: RegExp) {
  await act(async () => {
    fireEvent.click(screen.getByRole("tab", { name }));
  });
}

function resetTestDom() {
  cleanup();
  document.body.innerHTML = "";
  document.body.removeAttribute("data-scroll-locked");
  document.body.style.removeProperty("pointer-events");
}

function makeAppConfig(overrides: Partial<AppConfig_Serialize> = {}): AppConfig_Serialize {
  return {
    ConstItem: {
      RouteRulesTemplateSourceUrl: null,
    },
    UIItem: makeUiItem(),
    ...overrides,
  } as AppConfig_Serialize;
}

function makeUiItem(overrides: Partial<UiItem_Serialize> = {}): UiItem_Serialize {
  return {
    AutoHideStartup: false,
    ColorPrimaryName: "Teal",
    CurrentFontFamily: "",
    CurrentFontSize: 16,
    CurrentLanguage: "en",
    CurrentTheme: "FollowSystem",
    DoubleClick2Activate: false,
    EnableAutoAdjustMainLvColWidth: false,
    EnableDragDropSort: false,
    Hide2TrayWhenClose: false,
    MacOSShowInDock: false,
    MainColumnItem: [],
    MainGirdHeight1: 0,
    MainGirdHeight2: 0,
    MainGirdOrientation: 0,
    WindowSizeItem: [],
    ...overrides,
  };
}

function makeConnection(index: number, overrides: Partial<ClashConnectionItem> = {}): ClashConnectionItem {
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

function makeConnections(count: number): ClashConnectionItem[] {
  return Array.from({ length: count }, (_, index) => makeConnection(index));
}
