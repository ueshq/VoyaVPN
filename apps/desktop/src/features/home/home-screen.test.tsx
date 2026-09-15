import { useShellStore } from "@/stores/shell-store";
import { cleanup, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createTestQueryClient, renderWithQuery } from "@/test/render";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import type {
  ProfileListEntry,
  RuntimeStatusResponse,
  StatisticsSnapshot,
  SystemProxyStatusResponse,
  TunStatus,
} from "@/ipc/bindings";
import { IpcCommandError } from "@/ipc/commands";
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
  return {
    connectActiveProfile: vi.fn(),
    loadAppSettings: vi.fn(),
    getSettingsApplyStatus: vi.fn(),
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
  state: "disconnected",
};

const connectedStatus: RuntimeStatusResponse = {
  activeProfileId: "node-tokyo",
  mainPid: 4242,
  prePid: null,
  connectedDurationMs: null,
  activeTunBackend: null,
  state: "connected",
};

const sysProxyStatus: SystemProxyStatusResponse = {
  management: "automatic",
  effectiveMode: "forcedClear",
  exceptions: "",
  proxy: null,
  requestedMode: "forcedChange",
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

// The real error class and kind check: the sudo-retry and missing-core paths
// branch on `appError.kind`.
vi.mock("@/ipc/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/ipc/commands")>();
  return {
    appErrorOfKind: actual.appErrorOfKind,
    connectActiveProfile: ipcMock.connectActiveProfile,
    loadAppSettings: ipcMock.loadAppSettings,
    getSettingsApplyStatus: ipcMock.getSettingsApplyStatus,
    disconnectCore: ipcMock.disconnectCore,
    IpcCommandError: actual.IpcCommandError,
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
  };
});
vi.mock("@/ipc/runtime-event-store", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/ipc/runtime-event-store")>()),
  useRuntimeEventStore: runtimeMock.useRuntimeEventStore,
}));

// `listProfiles` answers with the rows plus the number of stored profiles this
// build could not decode; Home only reads the rows.
function mockProfileList(entries: ProfileListEntry[], undecodableProfiles = 0) {
  ipcMock.listProfiles.mockResolvedValue({ entries, undecodableProfiles });
}

function renderHome() {
  return renderWithQuery(<HomeScreen />, { queryClient: createTestQueryClient({ gcTime: 0 }) });
}

function connectButton() {
  return screen.getByTestId("home-connect-button");
}

describe("HomeScreen", () => {
  beforeEach(async () => {
    useShellStore.getState().setActiveTab("home");
    useRuntimeActionStore.setState({
      lastError: null,
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
    ipcMock.getSettingsApplyStatus.mockResolvedValue({ action: "none", connected: false });
    ipcMock.disconnectCore.mockResolvedValue(disconnectedStatus);
    ipcMock.restartCore.mockResolvedValue(connectedStatus);
    ipcMock.listPolicyGroups.mockResolvedValue({ entries: [] });
    ipcMock.policyGroupRuntime.mockResolvedValue(null);
    ipcMock.runtimeStatus.mockResolvedValue(disconnectedStatus);
    mockProfileList([
      makeActiveProfile({ id: "active", remarks: "Active node" }),
    ]);
    ipcMock.setActiveProfile.mockResolvedValue(makeProfile(0));
    ipcMock.systemProxyStatus.mockResolvedValue(sysProxyStatus);
    ipcMock.tunRequestElevation.mockResolvedValue(tunStatusResponse);
    ipcMock.tunStatus.mockResolvedValue(tunStatusResponse);
    useToastStore.setState({ toasts: [] });
    useModalStore.setState({ missingCore: null });
  });

  afterEach(async () => {
    cleanup();
    await changeLocale("en", { persist: false });
  });

  it("offers one add-node action when no nodes are available", async () => {
    mockProfileList([]);
    renderHome();
    await waitFor(() => expect(connectButton()).toBeEnabled());
    expect(connectButton()).toHaveAccessibleName("Add node");
    expect(screen.getAllByRole("button")).toEqual([connectButton()]);
    expect(screen.queryByRole("heading")).not.toBeInTheDocument();
    expect(screen.queryByText("Not protected")).not.toBeInTheDocument();
    expect(screen.queryByText("Add a node to connect")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Details" }),
    ).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Import" })).not.toBeInTheDocument();
    expect(screen.queryByTestId("home-connected-info")).not.toBeInTheDocument();
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Traffic mode" })).not.toBeInTheDocument();
  });

  it("states the connection in words and marks where traffic leaves", async () => {
    runtimeMock.state.coreState = connectedStatus;
    mockProfileList([
      makeActiveProfile({ id: "node-tokyo", remarks: "🇯🇵 Tokyo Edge" }),
    ]);
    const { container } = renderHome();
    await screen.findByRole("heading", { name: "Tokyo Edge" });
    const status = screen.getByTestId("home-status");
    expect(status).toHaveAttribute("role", "status");
    expect(status).toHaveTextContent("Connected");
    // No country was measured, so the flag in the name places the marker.
    expect(
      container.querySelector('.home-map-marker[data-country="JP"]'),
    ).toHaveAttribute("data-state", "connected");
  });

  it("marks the selected node's country before connecting", async () => {
    mockProfileList([
      makeActiveProfile({ id: "active", remarks: "🇸🇬 Singapore" }),
    ]);
    const { container } = renderHome();
    await screen.findByRole("heading", { name: "Singapore" });
    expect(screen.getByTestId("home-status")).toHaveTextContent("Disconnected");
    expect(
      container.querySelector('.home-map-marker[data-country="SG"]'),
    ).toHaveAttribute("data-state", "selected");
  });

  it("guides an empty home without a status headline or a map marker", async () => {
    mockProfileList([]);
    const { container } = renderHome();
    await waitFor(() => expect(connectButton()).toBeEnabled());
    expect(screen.getByTestId("home-status")).toHaveTextContent(
      "Add a subscription or paste a link to get started",
    );
    // Adding is not connecting, so the button does not show a power glyph.
    expect(connectButton().querySelector(".lucide-plus")).not.toBeNull();
    expect(connectButton().querySelector(".lucide-power")).toBeNull();
    expect(container.querySelector(".home-map-marker")).toBeNull();
  });

  it("points at the Rules page while global mode skips every rule", async () => {
    const user = userEvent.setup();
    const settings = makeAppSettings();
    ipcMock.loadAppSettings.mockResolvedValue({
      ...settings,
      proxy: { ...settings.proxy, trafficMode: "global" },
    });
    renderHome();

    const chip = await screen.findByRole("button", { name: "Routing setting: Global proxy" });
    // A pointer, not a control: Home still offers no way to change the mode.
    expect(screen.queryByRole("group", { name: "Traffic mode" })).not.toBeInTheDocument();
    await user.click(chip);
    expect(useShellStore.getState().activeTab).toBe("rules");
  });

  it("names the saved routing setting in rule mode", async () => {
    renderHome();
    await screen.findByRole("heading", { name: "Active node" });
    await waitFor(() => expect(ipcMock.loadAppSettings).toHaveBeenCalled());
    expect(screen.getByRole("button", { name: "Routing setting: Rules" })).toBeInTheDocument();
  });

  it("separates the actual capture path from saved VPN and routing settings", async () => {
    runtimeMock.state.coreState = connectedStatus;
    runtimeMock.state.tun = { ...tunStatusResponse, enabled: true };
    runtimeMock.state.sysProxy = { ...sysProxyStatus, effectiveMode: "forcedChange" };
    const settings = makeAppSettings();
    settings.proxy.trafficMode = "global";
    ipcMock.loadAppSettings.mockResolvedValue(settings);
    ipcMock.getSettingsApplyStatus.mockResolvedValue({ connected: true, action: "reconnect" });
    renderHome();
    expect(await screen.findByRole("button", { name: "Current capture: System proxy" })).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Routing setting: Global proxy" })).toBeInTheDocument();
    expect(await screen.findByText("Saved changes are waiting to be applied to this connection.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Current capture: VPN mode" })).not.toBeInTheDocument();
  });

  it("confirms a running native VPN from its backend and links to connection settings", async () => {
    runtimeMock.state.coreState = { ...connectedStatus, activeTunBackend: "macosPacketTunnel" };
    runtimeMock.state.tun = tunStatusResponse;
    runtimeMock.state.sysProxy = sysProxyStatus;
    renderHome();
    await userEvent.click(await screen.findByRole("button", { name: "Current capture: VPN mode" }));
    expect(useShellStore.getState()).toMatchObject({ activeTab: "settings", settingsTab: "connection" });
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
    expect(screen.getByText("In use · Policy group")).toBeInTheDocument();
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
    expect(screen.queryByRole("button", { name: "Details" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Reconnect" }));
    await waitFor(() => expect(ipcMock.restartCore).toHaveBeenCalledOnce());
    await user.click(screen.getByRole("button", { name: "Switch node" }));
    expect(useShellStore.getState()).toMatchObject({
      activeTab: "profiles",
      focusPageTitle: true,
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("keeps the disconnect action available while proxy capabilities are unavailable", () => {
    runtimeMock.state.coreState = connectedStatus;
    const view = renderHome();
    expect(screen.queryByText("Protection status unknown")).not.toBeInTheDocument();
    // Home states the connection itself, never a claim about protection.
    expect(screen.getByTestId("home-status")).toHaveTextContent("Connected");
    expect(connectButton()).toHaveAccessibleName("Disconnect");
    expect(ipcMock.systemProxyStatus).not.toHaveBeenCalled();
    view.unmount();
  });

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
      new IpcCommandError({
        kind: {
          candidates: [],
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

    await waitFor(() =>
      expect(useModalStore.getState().missingCore).toEqual({
        message: "sing-box is not installed",
      }),
    );
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });

  it("requests system authorization once and retries a connect that needed it", async () => {
    mockProfileList([
      makeActiveProfile({ id: "tokyo", remarks: "Tokyo Edge" }),
    ]);
    ipcMock.connectActiveProfile
      .mockRejectedValueOnce(
        new IpcCommandError({
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
    expect(useModalStore.getState().missingCore).toBeNull();
  });

  it("explains a declined authorization dialog instead of the raw failure", async () => {
    mockProfileList([
      makeActiveProfile({ id: "tokyo", remarks: "Tokyo Edge" }),
    ]);
    ipcMock.connectActiveProfile.mockRejectedValue(
      new IpcCommandError({
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

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(
      "Could not connect: System authorization was not granted, so VoyaVPN could not connect.",
    );
    expect(alert).not.toHaveTextContent("sudo helper refused");
    expect(useToastStore.getState().toasts).toHaveLength(0);
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
      state: "connected",
    });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not disconnect: sudo kill failed",
    );
    expect(connectButton()).toBeEnabled();

    await user.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(ipcMock.disconnectCore).toHaveBeenCalledTimes(2));
    await user.click(await screen.findByRole("button", { name: "View logs" }));
    expect(useShellStore.getState()).toMatchObject({
      activeTab: "settings",
      settingsTab: "advanced",
      settingsTarget: "logs",
    });
  });

  it("opens the Nodes add menu without starting a connection", async () => {
    mockProfileList([]);
    renderHome();
    await waitFor(() => expect(connectButton()).toBeEnabled());
    expect(connectButton()).toHaveAccessibleName("Add node");
    await userEvent.click(connectButton());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(useShellStore.getState()).toMatchObject({
      activeTab: "profiles",
      profilesAddMenuOpen: true,
    });
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
    expect(ipcMock.setActiveProfile).not.toHaveBeenCalled();
  });

  it("explains a node list that could not be read and retries it", async () => {
    ipcMock.listProfiles.mockRejectedValueOnce(new Error("Profiles unavailable"));
    renderHome();

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Could not read the node list: Profiles unavailable",
    );
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("heading", { name: "Active node" })).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("names a missing selection instead of claiming there are no nodes", async () => {
    mockProfileList([{ ...makeActiveProfile({ id: "saved" }), isActive: false }]);
    renderHome();

    expect(await screen.findByRole("heading", { name: "No node selected" })).toBeInTheDocument();
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


  it("offers no traffic or capture mode controls", async () => {
    runtimeMock.state.coreState = disconnectedStatus;
    runtimeMock.state.tun = { ...tunStatusResponse, enabled: true };
    renderHome();

    await waitFor(() => expect(connectButton()).toBeEnabled());
    // The traffic mode lives on the Rules page, the capture mode in Settings.
    expect(screen.queryByRole("group", { name: "Traffic mode" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Global" })).not.toBeInTheDocument();
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
    expect(screen.queryByText("TUN mode")).not.toBeInTheDocument();
    expect(ipcMock.setConnectionMode).not.toHaveBeenCalled();
  });

  it("asks macOS users to allow the VPN configuration", async () => {
    runtimeMock.state.tun = {
      ...tunStatusResponse,
      backend: "macosPacketTunnel",
      enabled: true,
      providerState: "permissionRequired",
    };
    renderHome();

    expect(
      await screen.findByText(
        "macOS asks to add a VPN configuration when you connect. Choose Allow. If you chose Don't Allow before, allow VoyaVPN in System Settings → VPN.",
      ),
    ).toHaveAttribute("role", "status");
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
      message: "Tunnel service is not installed",
    },
  ] as const)(
    "keeps $backend diagnostics visible under the mode card",
    async ({ backend, providerState, message }) => {
      runtimeMock.state.tun = {
        ...tunStatusResponse,
        backend,
        enabled: true,
        lastProviderError: message,
        nativeComponentReady: false,
        providerState,
      };
      renderHome();

      expect(
        await screen.findByText((text) => text.endsWith(message)),
      ).toBeInTheDocument();
    },
  );

  it("prevents connecting while a capture mode change is pending", async () => {
    useRuntimeActionStore.setState({ modePending: true });
    renderHome();

    expect(connectButton()).toBeDisabled();
    await userEvent.click(connectButton());
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
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
