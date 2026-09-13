import { useShellStore } from "@/stores/shell-store";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import type {
  AppError,
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

vi.mock("@/ipc/commands", () => ({
  connectActiveProfile: ipcMock.connectActiveProfile,
  loadAppSettings: ipcMock.loadAppSettings,
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
    useModalStore.setState({ stack: [] });
  });

  afterEach(async () => {
    cleanup();
    await changeLocale("en", { persist: false });
  });

  it("keeps only the connection control when no nodes are available", async () => {
    mockProfileList([]);
    renderHome();
    await waitFor(() => expect(connectButton()).toBeEnabled());
    expect(connectButton()).toHaveAccessibleName("Add node");
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

  it("points at the Rules page while global mode skips every rule", async () => {
    const user = userEvent.setup();
    const settings = makeAppSettings();
    ipcMock.loadAppSettings.mockResolvedValue({
      ...settings,
      proxy: { ...settings.proxy, trafficMode: "global" },
    });
    renderHome();

    const chip = await screen.findByRole("button", { name: "Global proxy" });
    expect(chip).toHaveAttribute(
      "title",
      "Global mode is on: all captured traffic goes through the proxy and these rules are skipped.",
    );
    // A pointer, not a control: Home still offers no way to change the mode.
    expect(screen.queryByRole("group", { name: "Traffic mode" })).not.toBeInTheDocument();
    await user.click(chip);
    expect(useShellStore.getState().activeTab).toBe("rules");
  });

  it("shows no mode reminder in rule mode", async () => {
    renderHome();
    await screen.findByRole("heading", { name: "Active node" });
    await waitFor(() => expect(ipcMock.loadAppSettings).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: "Global proxy" })).not.toBeInTheDocument();
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
    expect(screen.queryByText("Connected")).not.toBeInTheDocument();
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

  it("explains a declined authorization dialog instead of the raw failure", async () => {
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
      runningCoreType: "singBox",
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
    });
  });

  it("sends an empty home straight to the Nodes Add menu without starting a connection", async () => {
    mockProfileList([]);
    renderHome();
    await waitFor(() => expect(connectButton()).toBeEnabled());
    expect(connectButton()).toHaveAccessibleName("Add node");
    await userEvent.click(connectButton());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(useShellStore.getState()).toMatchObject({
      activeTab: "profiles", profilesAddMenuOpen: true, focusPageTitle: false,
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
