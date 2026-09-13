import { useShellStore } from "@/stores/shell-store";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import type {
  AppError,
  ConnectionModeStatus,
  ProfileListEntry,
  RuntimeStatusResponse,
  StatisticsSnapshot,
  SystemProxyStatusResponse,
  TunStatus,
} from "@/ipc/bindings";
import { useModalStore } from "@/stores/modal-store";
import { useRuntimeActionStore } from "@/stores/runtime-action-store";
import { useToastStore } from "@/stores/toast-store";
import { makeAppSettings } from "@/features/settings/app-settings.test-fixture";
import { makeProfileFixture } from "@/test/profile-fixture";

import { HomeScreen } from "./home-screen";

type RuntimeState = {
  coreState: RuntimeStatusResponse | null;
  coreStateReceivedAt: number | null;
  setCoreState: (state: RuntimeStatusResponse) => void;
  statistics: StatisticsSnapshot | null;
  sysProxy: SystemProxyStatusResponse | null;
  setSysProxy: (state: SystemProxyStatusResponse) => void;
  tun: TunStatus | null;
  setTun: (state: TunStatus) => void;
};

const runtimeMock = vi.hoisted(() => {
  const state: RuntimeState = {
    coreState: null,
    coreStateReceivedAt: null,
    setCoreState: vi.fn(),
    statistics: null,
    sysProxy: null,
    setSysProxy: vi.fn(),
    tun: null,
    setTun: vi.fn(),
  };
  const useRuntimeEventStore = Object.assign(
    (selector: (value: RuntimeState) => unknown) => selector(state),
    { getState: () => state },
  );

  return { state, useRuntimeEventStore };
});

const ipcMock = vi.hoisted(() => {
  // Faithful stand-in for the real error class: `runWithElevation` and
  // `missingCorePayload` branch on `appError.kind`, so a bare
  // `class extends Error {}` makes the sudo-retry and missing-core paths
  // unreachable from this suite.
  class MockIpcCommandError extends Error {
    readonly appError: AppError;

    constructor(appError: AppError) {
      super(appError.message);
      this.appError = appError;
      this.name = "IpcCommandError";
    }
  }

  return {
    IpcCommandError: MockIpcCommandError,
    connectActiveProfile: vi.fn(),
    loadAppSettings: vi.fn(),
    proxySetTrafficMode: vi.fn(),
    disconnectCore: vi.fn(),
    listPolicyGroups: vi.fn(),
    listProfiles: vi.fn(),
    policyGroupRuntime: vi.fn(),
    restartCore: vi.fn(),
    runtimeStatus: vi.fn(),
    setActiveProfile: vi.fn(),
    setConnectionMode: vi.fn(),
    systemProxyStatus: vi.fn(),
    tunRequestElevation: vi.fn(),
    tunStatus: vi.fn(),
  };
});

const disconnectedStatus: RuntimeStatusResponse = {
  activeProfileId: null,
  mainPid: null,
  prePid: null,
  connectedDurationMs: null,
  activeTunBackend: null,
  runningCoreType: null,
  state: "disconnected",
};

const connectedStatus: RuntimeStatusResponse = {
  activeProfileId: "node-tokyo",
  mainPid: 4242,
  prePid: null,
  connectedDurationMs: null,
  activeTunBackend: null,
  runningCoreType: "singBox",
  state: "connected",
};

const sysProxyStatus: SystemProxyStatusResponse = {
  management: "automatic",
  observation: "unknown",
  manualCleanupRequired: false,
  effectiveMode: "forcedClear",
  exceptions: "",
  proxy: null,
  requestedMode: "forcedChange",
};

const connectionModeStatus: ConnectionModeStatus = {
  mode: "systemProxy",
  processRulesEffective: false,
  vpnAvailable: true,
};

const tunStatusResponse: TunStatus = {
  allowEnableTun: true,
  backend: "process",
  enabled: false,
  elevationGranted: true,
  lastProviderError: null,
  nativeComponentReady: true,
  needsServiceInstall: false,
  needsVpnPermission: false,
  preflight: {
    notes: [],
    platform: "macos",
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
};

const missingTunnelMessages = {
  en: "The running copy of VoyaVPN is missing its VPN extension. Quit and open the fully installed app from Applications. If the extension is still missing, reinstall VoyaVPN.",
  "zh-Hans":
    "当前运行的 VoyaVPN 缺少 VPN 扩展。请退出后从“应用程序”打开完整安装版；若仍提示缺失，请重新安装。",
};

vi.mock("@/ipc/commands", () => ({
  connectActiveProfile: ipcMock.connectActiveProfile,
  loadAppSettings: ipcMock.loadAppSettings,
  proxySetTrafficMode: ipcMock.proxySetTrafficMode,
  disconnectCore: ipcMock.disconnectCore,
  IpcCommandError: ipcMock.IpcCommandError,
  listPolicyGroups: ipcMock.listPolicyGroups,
  listProfiles: ipcMock.listProfiles,
  policyGroupRuntime: ipcMock.policyGroupRuntime,
  restartCore: ipcMock.restartCore,
  runtimeStatus: ipcMock.runtimeStatus,
  setActiveProfile: ipcMock.setActiveProfile,
  setConnectionMode: ipcMock.setConnectionMode,
  systemProxyStatus: ipcMock.systemProxyStatus,
  tunRequestElevation: ipcMock.tunRequestElevation,
  tunStatus: ipcMock.tunStatus,
}));
vi.mock("@/ipc/runtime-event-store", () => ({ useRuntimeEventStore: runtimeMock.useRuntimeEventStore }));

// `listProfiles` answers with the rows plus the number of stored profiles this
// build could not decode; Home only reads the rows.
function mockProfileList(entries: ProfileListEntry[], undecodableProfiles = 0) {
  ipcMock.listProfiles.mockResolvedValue({ entries, undecodableProfiles });
}

function renderHome() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { gcTime: 0, retry: false } },
  });

  return {
    queryClient,
    ...render(
      <QueryClientProvider client={queryClient}>
        <HomeScreen />
      </QueryClientProvider>,
    ),
  };
}

function tunSwitch() {
  return screen.getByRole("switch", { name: /^(TUN mode|TUN模式)$/ });
}

function connectButton() {
  return screen.getByTestId("home-connect-button");
}

describe("HomeScreen", () => {
  beforeEach(async () => {
    useShellStore.getState().setActiveTab("home");
    useRuntimeActionStore.setState({
      pendingAction: null,
      modePending: false,
      switchingId: null,
    });
    await changeLocale("en", { persist: false });
    vi.clearAllMocks();
    runtimeMock.state.coreState = null;
    runtimeMock.state.statistics = null;
    runtimeMock.state.sysProxy = null;
    runtimeMock.state.tun = null;
    vi.mocked(runtimeMock.state.setTun).mockImplementation((status) => {
      runtimeMock.state.tun = status;
    });
    vi.mocked(runtimeMock.state.setSysProxy).mockImplementation((status) => {
      runtimeMock.state.sysProxy = status;
    });
    ipcMock.connectActiveProfile.mockResolvedValue(connectedStatus);
    ipcMock.loadAppSettings.mockResolvedValue(makeAppSettings());
    ipcMock.proxySetTrafficMode.mockResolvedValue({ mode: "rule" });
    ipcMock.disconnectCore.mockResolvedValue(disconnectedStatus);
    ipcMock.restartCore.mockResolvedValue(connectedStatus);
    ipcMock.listPolicyGroups.mockResolvedValue({ entries: [] });
    ipcMock.policyGroupRuntime.mockResolvedValue(null);
    ipcMock.runtimeStatus.mockResolvedValue(disconnectedStatus);
    mockProfileList([
      makeActiveProfile({ id: "active", remarks: "Active node" }),
    ]);
    ipcMock.setActiveProfile.mockResolvedValue(makeProfile(0));
    ipcMock.setConnectionMode.mockResolvedValue(connectionModeStatus);
    ipcMock.systemProxyStatus.mockResolvedValue(sysProxyStatus);
    ipcMock.tunRequestElevation.mockResolvedValue(tunStatusResponse);
    ipcMock.tunStatus.mockResolvedValue(tunStatusResponse);
    useToastStore.setState({ toasts: [] });
    useModalStore.setState({ stack: [] });
  });

  afterEach(async () => {
    cleanup();
    await changeLocale("en", { persist: false });
  });

  it("keeps only connection and mode controls when no nodes are available", async () => {
    mockProfileList([]);
    renderHome();
    await waitFor(() => expect(connectButton()).toBeEnabled());
    expect(connectButton()).toHaveAccessibleName("Connect");
    expect(screen.queryByRole("heading")).not.toBeInTheDocument();
    expect(screen.queryByText("Not protected")).not.toBeInTheDocument();
    expect(screen.queryByText("Add a node to connect")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Details" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Import" })).not.toBeInTheDocument();
    expect(screen.queryByTestId("home-connected-info")).not.toBeInTheDocument();
    expect(tunSwitch()).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Traffic mode" })).toBeInTheDocument();
  });

  it("shows the active policy group and the member traffic goes through", async () => {
    runtimeMock.state.coreState = connectedStatus;
    ipcMock.runtimeStatus.mockResolvedValue(connectedStatus);
    ipcMock.listPolicyGroups.mockResolvedValue({
      entries: [
        {
          group: {
            autoCreated: false,
            id: "g1",
            intervalSeconds: null,
            memberIds: ["node-tokyo"],
            name: "Asia",
            selectedProfileId: null,
            sourceSubscriptionId: null,
            strategy: "urlTest",
            testUrl: null,
            toleranceMs: null,
          },
          isActive: true,
          members: [{ profileId: "node-tokyo", remarks: "Tokyo" }],
        },
      ],
    });
    ipcMock.policyGroupRuntime.mockResolvedValue({
      groupId: "g1",
      members: [{ delayMs: 88, profileId: "node-tokyo", remarks: "Tokyo" }],
      nowProfileId: "node-tokyo",
    });
    renderHome();

    expect(await screen.findByRole("heading", { name: "Asia" })).toBeInTheDocument();
    expect(await screen.findByText("Via Tokyo")).toBeInTheDocument();
    expect(screen.getByText("Current policy group")).toBeInTheDocument();
    expect(screen.getByText("Lowest latency")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Details" })).not.toBeInTheDocument();
  });

  it("supports keyboard navigation to nodes without starting a connection", async () => {
    renderHome();
    await screen.findByRole("heading", { name: "Active node" });
    screen.getByRole("button", { name: "Switch node" }).focus();
    await userEvent.keyboard("{Enter}");
    expect(useShellStore.getState()).toMatchObject({
      activeTab: "profiles",
      focusPageTitle: true,
    });
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
    expect(ipcMock.setActiveProfile).not.toHaveBeenCalled();
  });

  it("shows the running node and navigates directly to nodes without connecting", async () => {
    runtimeMock.state.sysProxy = sysProxyStatus;
    runtimeMock.state.coreState = connectedStatus;
    mockProfileList([
      makeActiveProfile({ id: "node-tokyo", remarks: "Tokyo Edge" }),
    ]);
    const user = userEvent.setup();
    renderHome();
    expect(
      screen.queryByRole("heading", { level: 1 }),
    ).not.toBeInTheDocument();
    expect(connectButton()).toHaveAttribute("aria-pressed", "true");
    expect(connectButton()).toHaveAccessibleName("Disconnect");
    expect(
      await screen.findByRole("heading", { name: "Tokyo Edge" }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Details" }));
    expect(
      screen.getByRole("dialog", { name: "Connection details" }),
    ).toHaveTextContent("4242");
    expect(screen.getByRole("button", { name: "Restart" })).toBeEnabled();
    await user.keyboard("{Escape}");
    expect(screen.getByRole("button", { name: "Details" })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Switch node" }));
    expect(useShellStore.getState()).toMatchObject({
      activeTab: "profiles",
      focusPageTitle: true,
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("keeps manual proxy setup in settings and preserves unknown configuration", async () => {
    runtimeMock.state.coreState = connectedStatus;
    runtimeMock.state.sysProxy = {
      ...sysProxyStatus,
      management: "manual",
      observation: "unknown",
      requestedMode: "forcedChange",
      effectiveMode: "unchanged",
      proxy: "127.0.0.1:10808",
    };
    renderHome();
    expect(screen.queryByText("Local proxy ready")).not.toBeInTheDocument();
    expect(screen.queryByText("Connected")).not.toBeInTheDocument();
    expect(connectButton()).toHaveAccessibleName("Disconnect");
    expect(connectButton()).toHaveAttribute("aria-pressed", "true");
    expect(screen.queryByText("Manual proxy setup")).not.toBeInTheDocument();
    expect(
      screen.queryByText(
        "Configure your system proxy manually to use the local listener.",
      ),
    ).not.toBeInTheDocument();
    expect(screen.queryByTestId("manual-proxy-panel")).not.toBeInTheDocument();
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });

  it("keeps the disconnect action available while proxy capabilities are unavailable", () => {
    runtimeMock.state.coreState = connectedStatus;
    const view = renderHome();
    expect(screen.queryByText("Protection status unknown")).not.toBeInTheDocument();
    expect(screen.queryByText("Connected")).not.toBeInTheDocument();
    expect(connectButton()).toHaveAccessibleName("Disconnect");
    expect(ipcMock.systemProxyStatus).not.toHaveBeenCalled();
    view.unmount();
  });

  it.each([null, "macosPacketTunnel"] as const)(
    "shows the saved TUN choice independently of the running tunnel (%s)",
    (activeTunBackend) => {
      runtimeMock.state.coreState = { ...connectedStatus, activeTunBackend };
      runtimeMock.state.sysProxy = { ...sysProxyStatus, management: "manual" };
      runtimeMock.state.tun = {
        ...tunStatusResponse,
        enabled: true,
        backend: "macosPacketTunnel",
      };
      renderHome();
      expect(tunSwitch()).toBeChecked();
      expect(connectButton()).toHaveAccessibleName("Disconnect");
      expect(screen.queryByRole("heading", { level: 1 })).not.toBeInTheDocument();
    },
  );

  it("offers a retry when native tunnel cleanup is pending and refreshes TUN after failure", async () => {
    const pending = { ...connectedStatus, state: "cleanupPending" as const };
    runtimeMock.state.coreState = pending;
    ipcMock.runtimeStatus.mockResolvedValue(pending);
    ipcMock.disconnectCore.mockRejectedValue(new Error("stop timed out"));
    const user = userEvent.setup();
    renderHome();
    expect(connectButton()).toHaveAccessibleName("Retry disconnect");
    await waitFor(() => expect(connectButton()).toBeEnabled());
    await user.click(connectButton());
    await waitFor(() => expect(ipcMock.disconnectCore).toHaveBeenCalledOnce());
    await waitFor(() =>
      expect(runtimeMock.state.setCoreState).toHaveBeenCalledWith(pending),
    );
    expect(ipcMock.tunStatus).toHaveBeenCalled();
    expect(connectButton()).toBeEnabled();
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("invokes the connect action from the central button", async () => {
    const user = userEvent.setup();

    renderHome();

    await waitFor(() => expect(connectButton()).toBeEnabled());
    await user.click(connectButton());

    expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1);
    expect(ipcMock.disconnectCore).not.toHaveBeenCalled();
  });

  it("opens the missing-core recovery modal instead of a toast", async () => {
    mockProfileList([
      makeActiveProfile({ id: "tokyo", remarks: "Tokyo Edge" }),
    ]);
    ipcMock.connectActiveProfile.mockRejectedValue(
      new ipcMock.IpcCommandError({
        kind: {
          candidates: [],
          coreType: "singBox",
          downloadUrl: "https://example.test/core",
          searchDir: "/cores",
          type: "missingCore",
        },
        message: "sing-box is not installed",
        subsystem: "runtime",
      }),
    );

    const user = userEvent.setup();
    renderHome();
    await screen.findByRole("heading", { name: "Tokyo Edge" });
    await waitFor(() => expect(connectButton()).toBeEnabled());
    await user.click(connectButton());

    await waitFor(() => expect(useModalStore.getState().stack).toHaveLength(1));
    expect(useModalStore.getState().stack[0]).toMatchObject({
      kind: "missingCore",
      missingCore: {
        coreType: "singBox",
        message: "sing-box is not installed",
      },
    });
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });

  it("requests system authorization once and retries a connect that needed it", async () => {
    mockProfileList([
      makeActiveProfile({ id: "tokyo", remarks: "Tokyo Edge" }),
    ]);
    ipcMock.connectActiveProfile
      .mockRejectedValueOnce(
        new ipcMock.IpcCommandError({
          kind: { type: "elevationRequired" },
          message:
            "system authorization is required before enabling TUN on Unix",
          subsystem: "tun",
        }),
      )
      .mockResolvedValue(connectedStatus);
    ipcMock.tunRequestElevation.mockResolvedValue({
      ...tunStatusResponse,
      elevationGranted: true,
    });

    const user = userEvent.setup();
    renderHome();
    await screen.findByRole("heading", { name: "Tokyo Edge" });
    await waitFor(() => expect(connectButton()).toBeEnabled());
    await user.click(connectButton());

    await waitFor(() =>
      expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(2),
    );
    expect(ipcMock.tunRequestElevation).toHaveBeenCalledTimes(1);
    expect(useToastStore.getState().toasts).toHaveLength(0);
    expect(useModalStore.getState().stack).toHaveLength(0);
  });

  it("keeps the original failure when the authorization dialog is declined", async () => {
    mockProfileList([
      makeActiveProfile({ id: "tokyo", remarks: "Tokyo Edge" }),
    ]);
    ipcMock.connectActiveProfile.mockRejectedValue(
      new ipcMock.IpcCommandError({
        kind: { type: "elevationRequired" },
        message: "sudo helper refused",
        subsystem: "tun",
      }),
    );
    ipcMock.tunRequestElevation.mockResolvedValue({
      ...tunStatusResponse,
      elevationGranted: false,
    });

    const user = userEvent.setup();
    renderHome();
    await screen.findByRole("heading", { name: "Tokyo Edge" });
    await waitFor(() => expect(connectButton()).toBeEnabled());
    await user.click(connectButton());

    await waitFor(() =>
      expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
        description: "sudo helper refused",
        severity: "error",
      }),
    );
    expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1);
  });

  it("refreshes runtime state and surfaces errors when disconnect fails", async () => {
    const user = userEvent.setup();
    const disconnectError = new Error("sudo kill failed");
    runtimeMock.state.coreState = connectedStatus;
    ipcMock.disconnectCore.mockRejectedValue(disconnectError);
    ipcMock.runtimeStatus.mockResolvedValue(connectedStatus);

    renderHome();

    await waitFor(() => expect(connectButton()).toBeEnabled());
    await user.click(connectButton());

    await waitFor(() => expect(ipcMock.runtimeStatus).toHaveBeenCalledTimes(1));
    expect(runtimeMock.state.setCoreState).toHaveBeenCalledWith({
      activeProfileId: "node-tokyo",
      mainPid: 4242,
      prePid: null,
      connectedDurationMs: null,
      activeTunBackend: null,
      runningCoreType: "singBox",
      state: "connected",
    });
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "sudo kill failed",
      title: "Disconnect",
    });
    expect(connectButton()).toBeEnabled();
  });

  it("guides an empty home to the Nodes Add menu without starting a connection", async () => {
    mockProfileList([]);
    renderHome();
    await waitFor(() => expect(connectButton()).toBeEnabled());
    await userEvent.click(connectButton());
    const dialog = screen.getByRole("dialog", { name: "Add a node first" });
    expect(dialog).toHaveAccessibleDescription(
      "No nodes are available. Add a node or subscription before connecting.",
    );
    expect(useShellStore.getState().activeTab).toBe("home");
    await userEvent.click(within(dialog).getByRole("button", { name: "Add node" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(useShellStore.getState()).toMatchObject({
      activeTab: "profiles", profilesAddMenuOpen: true, focusPageTitle: false,
    });
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
    expect(ipcMock.setActiveProfile).not.toHaveBeenCalled();
  });

  it.each(["Cancel", "Close", "Escape"])("dismisses the node guide with %s and restores focus", async (action) => {
    mockProfileList([]);
    renderHome();
    await waitFor(() => expect(connectButton()).toBeEnabled());
    await userEvent.click(connectButton());
    const dialog = screen.getByRole("dialog", { name: "Add a node first" });
    if (action === "Escape") await userEvent.keyboard("{Escape}");
    else await userEvent.click(within(dialog).getByRole("button", { name: action }));
    await waitFor(() => expect(connectButton()).toHaveFocus());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(useShellStore.getState()).toMatchObject({ activeTab: "home", profilesAddMenuOpen: false });
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("does not offer the empty-node guide while profiles are loading", async () => {
    ipcMock.listProfiles.mockImplementation(() => new Promise<never>(() => {}));
    renderHome();
    expect(connectButton()).toBeDisabled();
    await userEvent.click(connectButton());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
  });

  it.each(["query error", "no selection"])("navigates to Nodes without the empty-node guide for %s", async (state) => {
    if (state === "query error") ipcMock.listProfiles.mockRejectedValue(new Error("Profiles unavailable"));
    else mockProfileList([{ ...makeActiveProfile({ id: "saved" }), isActive: false }]);
    renderHome();
    await waitFor(() => expect(connectButton()).toBeEnabled());
    expect(connectButton()).toHaveAccessibleName("Choose a node");
    await userEvent.click(connectButton());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(useShellStore.getState()).toMatchObject({ activeTab: "profiles", profilesAddMenuOpen: false });
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
  });

  it.each(["connected", "cleanupPending"] as const)("still disconnects an empty profile list while %s", async (state) => {
    runtimeMock.state.coreState = { ...connectedStatus, state };
    mockProfileList([]);
    renderHome();
    await userEvent.click(connectButton());
    await waitFor(() => expect(ipcMock.disconnectCore).toHaveBeenCalledOnce());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("turns saved TUN off from the keyboard and preserves the system proxy preference", async () => {
    runtimeMock.state.tun = { ...tunStatusResponse, enabled: true };
    runtimeMock.state.sysProxy = {
      ...sysProxyStatus,
      requestedMode: "forcedChange",
    };
    ipcMock.systemProxyStatus.mockResolvedValue(runtimeMock.state.sysProxy);
    const user = userEvent.setup();
    renderHome();

    expect(tunSwitch()).toBeChecked();
    tunSwitch().focus();
    await user.keyboard(" ");
    await waitFor(() =>
      expect(ipcMock.setConnectionMode).toHaveBeenCalledWith("systemProxy"),
    );
    await waitFor(() => expect(tunSwitch()).not.toBeChecked());
    expect(ipcMock.tunRequestElevation).not.toHaveBeenCalled();
  });

  it("shows a backend-confirmed TUN change", async () => {
    ipcMock.setConnectionMode.mockImplementation(async () => {
      ipcMock.tunStatus.mockResolvedValue({
        ...tunStatusResponse,
        enabled: true,
      });
      return { ...connectionModeStatus, mode: "vpn" };
    });
    const user = userEvent.setup();
    renderHome();
    await user.click(tunSwitch());
    await waitFor(() => expect(tunSwitch()).toBeChecked());
  });

  it("offers one traffic mode control", async () => {
    runtimeMock.state.coreState = disconnectedStatus;
    renderHome();

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Smart routing" }),
      ).toBeEnabled(),
    );
    expect(
      screen.getByRole("group", { name: "Traffic mode" }),
    ).toBeInTheDocument();
  });

  it.each([
    ["en", "About TUN mode", "TUN captures device traffic and may require system permission", "About traffic mode", "Smart routing: Rules determine which traffic uses the proxy;\nGlobal proxy: All captured traffic uses the selected node;"],
    ["zh-Hans", "TUN模式说明", "TUN接管设备流量，可能需要系统授权", "流量模式说明", "智能分流：根据规则决定哪些流量使用代理；\n全局代理：接管的流量均使用所选节点；"],
    ["zh-Hant", "TUN模式說明", "TUN接管裝置流量，可能需要系統授權", "流量模式說明", "智慧分流：根據規則決定哪些流量使用代理；\n全域代理：接管的流量均使用所選節點；"],
  ] as const)("shows localized mode help on hover and focus in %s without changing TUN", async (locale, tunLabel, tunHint, trafficLabel, trafficHint) => {
    await changeLocale(locale, { persist: false });
    const user = userEvent.setup();
    renderHome();
    await waitFor(() => expect(tunSwitch()).toBeEnabled());
    expect(screen.queryByText(tunHint)).not.toBeInTheDocument();
    expect(screen.queryByText(trafficHint)).not.toBeInTheDocument();

    const info = screen.getByRole("button", { name: tunLabel });
    await user.hover(info);
    expect((await screen.findByRole("tooltip")).textContent).toBe(tunHint);
    await user.click(info);
    expect(tunSwitch()).not.toBeChecked();
    expect(ipcMock.setConnectionMode).not.toHaveBeenCalled();
    await user.unhover(info);
    await user.tab();
    expect(tunSwitch()).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: trafficLabel })).toHaveFocus();
    expect((await screen.findByRole("tooltip")).textContent).toBe(trafficHint);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
    expect(ipcMock.setConnectionMode).not.toHaveBeenCalled();
    expect(ipcMock.proxySetTrafficMode).not.toHaveBeenCalled();
  });

  it("shows the backend reason and restores controls when mode switching fails", async () => {
    const user = userEvent.setup();
    ipcMock.setConnectionMode.mockRejectedValue(
      new Error("desktop policy rejected the mode"),
    );

    renderHome();
    const toggle = tunSwitch();
    await user.click(toggle);

    await waitFor(() =>
      expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
        description: "desktop policy rejected the mode",
        severity: "error",
        title: "Failed to change connection mode",
      }),
    );
    expect(toggle).toBeEnabled();
    expect(toggle).not.toBeChecked();
  });

  it("prevents duplicate mode submissions while one is pending", async () => {
    const user = userEvent.setup();
    let resolveMode: ((status: ConnectionModeStatus) => void) | undefined;
    ipcMock.setConnectionMode.mockImplementation(
      () =>
        new Promise<ConnectionModeStatus>((resolve) => {
          resolveMode = resolve;
        }),
    );

    renderHome();
    const toggle = tunSwitch();
    await user.click(toggle);
    expect(toggle).toBeDisabled();
    await user.click(toggle);
    await waitFor(() =>
      expect(ipcMock.setConnectionMode).toHaveBeenCalledTimes(1),
    );

    resolveMode?.({ ...connectionModeStatus, mode: "vpn" });
    await waitFor(() => expect(toggle).toBeEnabled());
  });

  it("refuses a mode switch while a connect is still in flight", async () => {
    const user = userEvent.setup();
    // Flipping TUN mid-connect persists the flag but cannot restart a core that
    // is not Connected yet, so the UI would claim TUN over a non-TUN core.
    let resolveConnect: ((status: RuntimeStatusResponse) => void) | undefined;
    ipcMock.connectActiveProfile.mockImplementation(
      () =>
        new Promise<RuntimeStatusResponse>((resolve) => {
          resolveConnect = resolve;
        }),
    );

    renderHome();
    await waitFor(() => expect(connectButton()).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "Connect" }));

    const toggle = tunSwitch();
    expect(toggle).toBeDisabled();
    await user.click(toggle);
    expect(ipcMock.setConnectionMode).not.toHaveBeenCalled();

    resolveConnect?.(connectedStatus);
    await waitFor(() =>
      expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1),
    );
  });

  it("prevents connecting during a pending mode transaction", async () => {
    let resolveMode: ((status: ConnectionModeStatus) => void) | undefined;
    ipcMock.setConnectionMode.mockImplementation(
      () =>
        new Promise<ConnectionModeStatus>((resolve) => {
          resolveMode = resolve;
        }),
    );
    const user = userEvent.setup();
    renderHome();
    await user.click(tunSwitch());
    expect(connectButton()).toBeDisabled();
    await user.click(connectButton());
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
    resolveMode?.(connectionModeStatus);
    await waitFor(() => expect(connectButton()).toBeEnabled());
  });

  it("keeps the current node honest when the running profile differs from the saved selection", async () => {
    runtimeMock.state.coreState = connectedStatus;
    mockProfileList([
      makeActiveProfile({ id: "other", remarks: "Other saved node" }),
    ]);
    const user = userEvent.setup();
    renderHome();
    expect(
      await screen.findByRole("heading", { name: "node-tokyo" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "Other saved node" }),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Switch node" }));
    expect(useShellStore.getState().activeTab).toBe("profiles");
    expect(ipcMock.setActiveProfile).not.toHaveBeenCalled();
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
    expect(ipcMock.restartCore).not.toHaveBeenCalled();
  });

  it("keeps a committed mode change successful when only the status refresh fails", async () => {
    const user = userEvent.setup();
    // The backend already persisted and emitted its post-commit events, so a
    // failing follow-up read must not be reported as a failed mode change.
    ipcMock.systemProxyStatus.mockRejectedValue(
      new Error("status read failed"),
    );

    renderHome();
    await user.click(tunSwitch());

    await waitFor(() =>
      expect(ipcMock.setConnectionMode).toHaveBeenCalledWith("vpn"),
    );
    await waitFor(() => {
      const titles = useToastStore
        .getState()
        .toasts.map((toast) => toast.title);
      expect(titles).toContain("Failed to read system proxy status");
      expect(titles).not.toContain("Failed to change connection mode");
    });
  });

  it("requests system authorization on demand before entering TUN mode", async () => {
    const user = userEvent.setup();
    ipcMock.tunStatus.mockResolvedValue({
      ...tunStatusResponse,
      requiresElevation: true,
      elevationGranted: false,
    });
    ipcMock.tunRequestElevation.mockResolvedValue({
      ...tunStatusResponse,
      requiresElevation: true,
      elevationGranted: true,
    });

    renderHome();

    await user.click(tunSwitch());

    await waitFor(() =>
      expect(ipcMock.tunRequestElevation).toHaveBeenCalledTimes(1),
    );
    await waitFor(() =>
      expect(ipcMock.setConnectionMode).toHaveBeenCalledWith("vpn"),
    );
  });

  it.each([
    {
      locale: "en",
      lastProviderError: "PacketTunnel extension is not bundled in this build",
    },
    { locale: "zh-Hans", lastProviderError: "A different backend diagnostic" },
    { locale: "en", lastProviderError: null },
  ] as const)(
    "explains how to restore the missing macOS extension in $locale ($lastProviderError)",
    async ({ locale, lastProviderError }) => {
      await changeLocale(locale, { persist: false });
      const user = userEvent.setup();
      ipcMock.tunStatus.mockResolvedValue({
        ...tunStatusResponse,
        backend: "macosPacketTunnel",
        lastProviderError,
        nativeComponentReady: false,
        providerState: "missingComponent",
        requiresElevation: true,
        elevationGranted: false,
      });

      renderHome();

      await user.click(tunSwitch());

      await waitFor(() =>
        expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
          description: missingTunnelMessages[locale],
          severity: "error",
          title: locale === "en" ? "Failed to enable TUN" : "启用 TUN 失败",
        }),
      );
      expect(ipcMock.setConnectionMode).not.toHaveBeenCalled();
      expect(ipcMock.tunRequestElevation).not.toHaveBeenCalled();
      expect(tunSwitch()).toHaveAttribute("aria-checked", "false");
    },
  );

  it.each(["en", "zh-Hans"] as const)(
    "shows the same recovery advice in the persisted TUN status in %s",
    async (locale) => {
      await changeLocale(locale, { persist: false });
      const status: TunStatus = {
        ...tunStatusResponse,
        backend: "macosPacketTunnel",
        enabled: true,
        nativeComponentReady: false,
        providerState: "missingComponent",
        lastProviderError:
          "PacketTunnel extension is not bundled in this build",
      };
      runtimeMock.state.tun = status;
      ipcMock.tunStatus.mockResolvedValue(status);

      renderHome();

      const summary = await screen.findByText((text) =>
        text.endsWith(missingTunnelMessages[locale]),
      );
      expect(summary).toHaveTextContent(
        locale === "en" ? "Missing component" : "缺少组件",
      );
      expect(summary).not.toHaveTextContent(
        "PacketTunnel extension is not bundled in this build",
      );
      expect(status.lastProviderError).toBe(
        "PacketTunnel extension is not bundled in this build",
      );
    },
  );

  it("allows TUN mode when the macOS extension is present", async () => {
    const user = userEvent.setup();
    ipcMock.tunStatus.mockResolvedValue({
      ...tunStatusResponse,
      backend: "macosPacketTunnel",
      providerState: "stopped",
    });

    renderHome();
    await user.click(tunSwitch());

    await waitFor(() =>
      expect(ipcMock.setConnectionMode).toHaveBeenCalledWith("vpn"),
    );
    expect(ipcMock.tunRequestElevation).not.toHaveBeenCalled();
    expect(useToastStore.getState().toasts).toEqual([]);
  });

  it.each([
    {
      backend: "macosPacketTunnel",
      providerState: "error",
      message: "PacketTunnel signature is invalid",
    },
    {
      backend: "windowsService",
      providerState: "missingComponent",
      message: "PacketTunnel extension is not bundled in this build",
    },
  ] as const)(
    "preserves other $backend diagnostics in notifications and status",
    async ({ backend, providerState, message }) => {
      const user = userEvent.setup();
      const status: TunStatus = {
        ...tunStatusResponse,
        backend,
        providerState,
        nativeComponentReady: false,
        lastProviderError: message,
      };
      ipcMock.tunStatus.mockResolvedValue(status);
      const { queryClient, rerender } = renderHome();

      await user.click(tunSwitch());
      await waitFor(() =>
        expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
          description: message,
        }),
      );
      expect(ipcMock.setConnectionMode).not.toHaveBeenCalled();

      runtimeMock.state.tun = { ...status, enabled: true };
      rerender(
        <QueryClientProvider client={queryClient}>
          <HomeScreen />
        </QueryClientProvider>,
      );
      expect(
        await screen.findByText((text) => text.endsWith(message)),
      ).toBeInTheDocument();
    },
  );

  it("localizes the generic missing-component fallback", async () => {
    await changeLocale("zh-Hans", { persist: false });
    const user = userEvent.setup();
    ipcMock.tunStatus.mockResolvedValue({
      ...tunStatusResponse,
      backend: "windowsService",
      providerState: "missingComponent",
      nativeComponentReady: false,
    });

    renderHome();
    await user.click(tunSwitch());
    await waitFor(() =>
      expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
        description: "尚未安装原生隧道组件。",
      }),
    );
    expect(ipcMock.setConnectionMode).not.toHaveBeenCalled();
  });

  it("blocks TUN mode when PlugInKit elected a stale provider path", async () => {
    const user = userEvent.setup();
    ipcMock.tunStatus.mockResolvedValue({
      ...tunStatusResponse,
      backend: "macosPacketTunnel",
      expectedProviderPath:
        "/Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
      providerPathMismatch: true,
      resolvedProviderPath:
        "/Users/afu/Dev/VoyaVPN/target/native/macos/runtime-kill-tests/profile-only.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
    });

    renderHome();

    await user.click(tunSwitch());

    await waitFor(() => expect(ipcMock.tunStatus).toHaveBeenCalled());
    expect(ipcMock.setConnectionMode).not.toHaveBeenCalled();
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: expect.stringContaining("pnpm native:macos:ne:doctor --fix"),
      title: "Failed to enable TUN",
    });
  });

  it("leaves the mode unchanged when the authorization dialog is cancelled", async () => {
    const user = userEvent.setup();
    ipcMock.tunStatus.mockResolvedValue({
      ...tunStatusResponse,
      requiresElevation: true,
      elevationGranted: false,
    });
    ipcMock.tunRequestElevation.mockResolvedValue({
      ...tunStatusResponse,
      requiresElevation: true,
      elevationGranted: false,
    });

    renderHome();

    await user.click(tunSwitch());

    await waitFor(() =>
      expect(ipcMock.tunRequestElevation).toHaveBeenCalledTimes(1),
    );
    expect(ipcMock.setConnectionMode).not.toHaveBeenCalled();
    await waitFor(() => expect(tunSwitch()).toBeEnabled());
    expect(tunSwitch()).not.toBeChecked();
  });
});

function makeActiveProfile(
  overrides: Parameters<typeof makeProfileFixture>[1] = {},
): ProfileListEntry {
  return { ...makeProfile(0, overrides), isActive: true };
}

function makeProfile(
  index: number,
  overrides: Parameters<typeof makeProfileFixture>[1] = {},
): ProfileListEntry {
  return makeProfileFixture(index, overrides, false);
}
