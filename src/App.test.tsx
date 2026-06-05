import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { vi } from "vitest";

import { App } from "./App";
import { fontToCss } from "./config/fonts";
import { changeLocale } from "./i18n";
import { loadAppConfig, saveAppConfig } from "@/ipc";
import type { AppConfig_Serialize, UiItem_Serialize } from "@/ipc/bindings";

vi.mock("@/ipc", () => ({
  connectActiveProfile: vi.fn(),
  EventBridge: () => null,
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
  clashStartMonitor: vi.fn(() => Promise.resolve({ running: true })),
  clashStopMonitor: vi.fn(() => Promise.resolve({ running: false })),
  clashTestDelay: vi.fn(() => Promise.resolve([])),
  copyProfiles: vi.fn(),
  dedupeProfiles: vi.fn(),
  deleteSubscriptions: vi.fn(),
  deleteProfiles: vi.fn(),
  deleteRoutingRules: vi.fn(),
  deleteRoutings: vi.fn(),
  disconnectCore: vi.fn(),
  generateQrCode: vi.fn(() => Promise.resolve({ mimeType: "image/svg+xml", svg: "<svg />" })),
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
      xrayDnsItem: {
        Id: "dns-xray",
        Remarks: "Xray",
        Enabled: false,
        CoreType: 2,
        UseSystemHosts: false,
      },
      singboxDnsItem: {
        Id: "dns-singbox",
        Remarks: "sing-box",
        Enabled: false,
        CoreType: 24,
        UseSystemHosts: false,
      },
      defaults: {
        xrayNormalDns: "{\"servers\":[]}",
        xrayTunDns: "{\"servers\":[]}",
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
      xrayRoutes: [],
      xrayBalancers: [],
      xrayObservatorySelectors: [],
      xrayBurstObservatorySelectors: [],
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
  setTunEnabled: vi.fn(() =>
    Promise.resolve({
      allowEnableTun: true,
      enabled: false,
      preflight: {
        notes: [],
        platform: "linux",
        routeRestoreNote: "",
        state: "ready",
        windowsCleanupDevices: [],
      },
      requiresSudoPassword: false,
      restoreOnDisconnect: true,
      sudoPasswordPresent: false,
    }),
  ),
  sortProfiles: vi.fn(),
  validateGroupProfile: vi.fn(() =>
    Promise.resolve({ childIndexIds: [], errors: [], normalizedChildItems: "", valid: true, warnings: [] }),
  ),
  sudoBeginCollection: vi.fn(() => Promise.resolve({ requestId: null, state: "ready" })),
  sudoClearPassword: vi.fn(),
  sudoSubmitPassword: vi.fn(),
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
      enabled: false,
      preflight: {
        notes: [],
        platform: "linux",
        routeRestoreNote: "",
        state: "ready",
        windowsCleanupDevices: [],
      },
      requiresSudoPassword: false,
      restoreOnDisconnect: true,
      sudoPasswordPresent: false,
    }),
  ),
  checkUpdates: vi.fn(() => Promise.resolve({ preRelease: false, results: [], targets: [] })),
  downloadUpdates: vi.fn(() => Promise.resolve({ preRelease: false, results: [], targets: [] })),
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
  useRuntimeEventStore: Object.assign(
    (selector: (state: unknown) => unknown) =>
      selector({
        clearLogs: vi.fn(),
        clashConnections: null,
        clashTraffic: null,
        coreState: null,
        lastTransientEvent: null,
        logLines: [],
        pushTransientEvent: vi.fn(),
        serverStatsByProfileId: {},
        setClashConnections: vi.fn(),
        setClashTraffic: vi.fn(),
        setCoreState: vi.fn(),
        setSysProxy: vi.fn(),
        setTun: vi.fn(),
        speedtestResultsByProfileId: {},
        statistics: null,
        sysProxy: null,
        tun: null,
      }),
    { getState: vi.fn() },
  ),
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
    window.localStorage.clear();
    document.documentElement.className = "";
    document.documentElement.style.removeProperty("--app-font-family");
    document.documentElement.style.removeProperty("--app-font-size");
    vi.mocked(loadAppConfig).mockClear();
    vi.mocked(saveAppConfig).mockClear();
    vi.mocked(loadAppConfig).mockResolvedValue(makeAppConfig());
    vi.mocked(saveAppConfig).mockImplementation(async (config) => config as AppConfig_Serialize);
    await changeLocale("en");
  });

  it("renders the app shell tabs and status bar", () => {
    renderApp();

    expect(screen.getByRole("heading", { name: "VoyaVPN" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Profiles/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Routing/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /DNS/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Clash Proxies/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Clash Connections/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /Logs/ })).toBeInTheDocument();
    expect(screen.getByTestId("status-bar")).toHaveTextContent("Disconnected");
  });

  it("switches document direction through the RTL locale", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "FA" }));

    await waitFor(() => expect(document.documentElement).toHaveAttribute("dir", "rtl"));
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
  });
});

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
