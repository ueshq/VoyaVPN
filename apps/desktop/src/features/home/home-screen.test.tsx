import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  AppError,
  ConnectionModeStatus,
  CoreStateEvent,
  ProfileListEntry,
  RuntimeStatusResponse,
  StatisticsSnapshot,
  SysProxyChanged,
  SystemProxyStatusResponse,
  TunChanged,
  TunStatus,
} from "@/ipc/bindings";
import { useModalStore } from "@/stores/modal-store";
import { useToastStore } from "@/stores/toast-store";
import { makeProfileFixture } from "@/test/profile-fixture";

import { HomeScreen } from "./home-screen";

type RuntimeState = {
  coreState: CoreStateEvent | null;
  setCoreState: (state: CoreStateEvent) => void;
  statistics: StatisticsSnapshot | null;
  sysProxy: SysProxyChanged | null;
  setSysProxy: (state: SysProxyChanged) => void;
  tun: TunChanged | null;
  setTun: (state: TunChanged) => void;
};

const runtimeMock = vi.hoisted(() => {
  const state: RuntimeState = {
    coreState: null,
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

    constructor(appError: AppError, message = "ipc failed") {
      super(message);
      this.appError = appError;
      this.name = "IpcCommandError";
    }
  }

  return {
    IpcCommandError: MockIpcCommandError,
    connectActiveProfile: vi.fn(),
    deleteSubscriptions: vi.fn(),
    disconnectCore: vi.fn(),
    listProfiles: vi.fn(),
    listSubscriptionMetadata: vi.fn(),
    listSubscriptions: vi.fn(),
    restartCore: vi.fn(),
    runtimeStatus: vi.fn(),
    saveSubscription: vi.fn(),
    setActiveProfile: vi.fn(),
    setConnectionMode: vi.fn(),
    systemProxyStatus: vi.fn(),
    tunRequestElevation: vi.fn(),
    tunStatus: vi.fn(),
    updateSubscriptions: vi.fn(),
  };
});

const disconnectedStatus: RuntimeStatusResponse = {
  activeProfileId: null,
  mainPid: null,
  prePid: null,
  runningCoreType: null,
  state: "disconnected",
};

const connectedStatus: RuntimeStatusResponse = {
  activeProfileId: "node-tokyo",
  mainPid: 4242,
  prePid: null,
  runningCoreType: "singBox",
  state: "connected",
};

const sysProxyStatus: SystemProxyStatusResponse = {
  effectiveMode: "forcedClear",
  exceptions: "",
  pacAvailable: false,
  pacUrl: null,
  proxy: null,
  requestedMode: "forcedClear",
};

const connectionModeStatus: ConnectionModeStatus = {
  mode: "proxyOnly",
  pacAvailable: false,
  pacEnabled: false,
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

vi.mock("@/ipc", () => ({
  connectActiveProfile: ipcMock.connectActiveProfile,
  deleteSubscriptions: ipcMock.deleteSubscriptions,
  disconnectCore: ipcMock.disconnectCore,
  IpcCommandError: ipcMock.IpcCommandError,
  listProfiles: ipcMock.listProfiles,
  listSubscriptionMetadata: ipcMock.listSubscriptionMetadata,
  listSubscriptions: ipcMock.listSubscriptions,
  restartCore: ipcMock.restartCore,
  runtimeStatus: ipcMock.runtimeStatus,
  saveSubscription: ipcMock.saveSubscription,
  setActiveProfile: ipcMock.setActiveProfile,
  setConnectionMode: ipcMock.setConnectionMode,
  systemProxyStatus: ipcMock.systemProxyStatus,
  tunRequestElevation: ipcMock.tunRequestElevation,
  tunStatus: ipcMock.tunStatus,
  updateSubscriptions: ipcMock.updateSubscriptions,
  useRuntimeEventStore: runtimeMock.useRuntimeEventStore,
}));

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

const connectedCoreState: CoreStateEvent = {
  activeProfileId: "node-tokyo",
  mainPid: 4242,
  prePid: null,
  runningCoreType: "singBox",
  state: "connected",
};

function connectButton() {
  return screen.getByTestId("home-connect-button");
}

describe("HomeScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runtimeMock.state.coreState = null;
    runtimeMock.state.statistics = null;
    runtimeMock.state.sysProxy = null;
    runtimeMock.state.tun = null;
    ipcMock.connectActiveProfile.mockResolvedValue(connectedStatus);
    ipcMock.disconnectCore.mockResolvedValue(disconnectedStatus);
    ipcMock.restartCore.mockResolvedValue(connectedStatus);
    ipcMock.runtimeStatus.mockResolvedValue(disconnectedStatus);
    ipcMock.listProfiles.mockResolvedValue([]);
    ipcMock.listSubscriptionMetadata.mockResolvedValue([]);
    ipcMock.listSubscriptions.mockResolvedValue([]);
    ipcMock.setActiveProfile.mockResolvedValue(makeProfile(0));
    ipcMock.setConnectionMode.mockResolvedValue(connectionModeStatus);
    ipcMock.systemProxyStatus.mockResolvedValue(sysProxyStatus);
    ipcMock.tunRequestElevation.mockResolvedValue(tunStatusResponse);
    ipcMock.tunStatus.mockResolvedValue(tunStatusResponse);
    useToastStore.setState({ toasts: [] });
    useModalStore.setState({ stack: [] });
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the calm unprotected hero with an empty node list by default", async () => {
    renderHome();

    expect(screen.getByRole("region", { name: "Connection home" })).toBeInTheDocument();
    expect(screen.getByText("Not protected")).toBeInTheDocument();
    const connect = connectButton();
    expect(connect).toBeEnabled();
    expect(connect).toHaveAttribute("aria-pressed", "false");
    expect(connect).toHaveAccessibleName("Connect");
    expect(await screen.findByText("No nodes available")).toBeInTheDocument();
  });

  it("lights up the protected state with node info and marks the running node", async () => {
    runtimeMock.state.coreState = connectedCoreState;
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ id: "node-tokyo", remarks: "Tokyo Edge" }),
    ]);

    renderHome();

    expect(screen.getByText("Protected")).toBeInTheDocument();
    expect(screen.getByTestId("home-status-card")).toHaveTextContent("PID 4242");
    expect(connectButton()).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Restart" })).toBeInTheDocument();
    expect(
      await screen.findByRole("button", { name: "Current node: Tokyo Edge" }),
    ).toBeInTheDocument();

    const row = await screen.findByRole("option", { name: /Tokyo Edge/ });
    // Blue selection is seeded to the active node; the green "live" dot marks the
    // node that is actually running.
    expect(row).toHaveAttribute("aria-selected", "true");
    expect(row.querySelector(".bg-connected")).not.toBeNull();
  });

  it("selects a node locally on single click without touching the backend", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ id: "osaka", remarks: "Osaka Edge" }),
      makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" }),
    ]);

    const user = userEvent.setup();
    renderHome();

    await user.click(await screen.findByRole("option", { name: /Tokyo Edge/ }));

    expect(screen.getByRole("option", { name: /Tokyo Edge/ })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("option", { name: /Osaka Edge/ })).toHaveAttribute("aria-selected", "false");
    expect(ipcMock.setActiveProfile).not.toHaveBeenCalled();
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
    expect(ipcMock.restartCore).not.toHaveBeenCalled();
  });

  it("switches and connects on double click while disconnected", async () => {
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" })]);

    const user = userEvent.setup();
    renderHome();

    await user.dblClick(await screen.findByRole("option", { name: /Tokyo Edge/ }));

    expect(ipcMock.setActiveProfile).toHaveBeenCalledWith("tokyo");
    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1));
    expect(ipcMock.restartCore).not.toHaveBeenCalled();
  });

  it("switches and restarts on double click while connected", async () => {
    runtimeMock.state.coreState = {
      activeProfileId: "node-old",
      mainPid: 1,
      prePid: null,
      runningCoreType: "singBox",
      state: "connected",
    };
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" })]);

    const user = userEvent.setup();
    renderHome();

    await user.dblClick(await screen.findByRole("option", { name: /Tokyo Edge/ }));

    expect(ipcMock.setActiveProfile).toHaveBeenCalledWith("tokyo");
    await waitFor(() => expect(ipcMock.restartCore).toHaveBeenCalledTimes(1));
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("activates the focused node on Enter", async () => {
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" })]);

    const user = userEvent.setup();
    renderHome();

    const tokyo = await screen.findByRole("option", { name: /Tokyo Edge/ });
    tokyo.focus();
    await user.keyboard("{Enter}");

    expect(ipcMock.setActiveProfile).toHaveBeenCalledWith("tokyo");
    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1));
  });

  it("keeps one tab stop and moves the active node with the arrow keys", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" }),
      makeProfile(2, { id: "osaka", remarks: "Osaka Edge" }),
      makeProfile(3, { id: "seoul", remarks: "Seoul Edge" }),
    ]);

    const user = userEvent.setup();
    renderHome();

    const tokyo = await screen.findByRole("option", { name: /Tokyo Edge/ });
    const osaka = screen.getByRole("option", { name: /Osaka Edge/ });
    const seoul = screen.getByRole("option", { name: /Seoul Edge/ });
    // Only one option is in the tab order; the rest are reached with arrows.
    expect(tokyo).toHaveAttribute("tabindex", "0");
    expect(osaka).toHaveAttribute("tabindex", "-1");

    tokyo.focus();
    await user.keyboard("{ArrowDown}");
    expect(osaka).toHaveAttribute("aria-selected", "true");
    expect(osaka).toHaveFocus();
    expect(osaka).toHaveAttribute("tabindex", "0");
    expect(tokyo).toHaveAttribute("tabindex", "-1");

    await user.keyboard("{End}");
    expect(seoul).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(osaka).toHaveFocus();
    await user.keyboard("{Home}");
    expect(tokyo).toHaveFocus();

    // Navigating is local: it never touches the backend.
    expect(ipcMock.setActiveProfile).not.toHaveBeenCalled();
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("invokes the connect action from the central button", async () => {
    const user = userEvent.setup();

    renderHome();

    await user.click(connectButton());

    expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1);
    expect(ipcMock.disconnectCore).not.toHaveBeenCalled();
  });

  it("connects to the locally selected node, switching the active profile first", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ id: "osaka", remarks: "Osaka Edge" }),
      makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" }),
    ]);

    const user = userEvent.setup();
    renderHome();

    await user.click(await screen.findByRole("option", { name: /Tokyo Edge/ }));
    await user.click(connectButton());

    expect(ipcMock.setActiveProfile).toHaveBeenCalledWith("tokyo");
    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1));
    // The active profile is switched before connect so the tunnel uses it.
    expect(ipcMock.setActiveProfile.mock.invocationCallOrder[0]).toBeLessThan(
      ipcMock.connectActiveProfile.mock.invocationCallOrder[0],
    );
  });

  it("connects directly when the selection already matches the active node", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ id: "osaka", remarks: "Osaka Edge" }),
    ]);

    const user = userEvent.setup();
    renderHome();

    await screen.findByRole("option", { name: /Osaka Edge/ });
    await user.click(connectButton());

    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1));
    expect(ipcMock.setActiveProfile).not.toHaveBeenCalled();
  });

  it("refuses node activation while a runtime action is still in flight", async () => {
    runtimeMock.state.coreState = connectedCoreState;
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ id: "node-tokyo", remarks: "Tokyo Edge" }),
      makeProfile(1, { id: "osaka", remarks: "Osaka Edge" }),
    ]);
    // Never settles: the disconnect stays pending for the whole test.
    ipcMock.disconnectCore.mockReturnValue(new Promise(() => {}));

    const user = userEvent.setup();
    renderHome();

    const osaka = await screen.findByRole("option", { name: /Osaka Edge/ });
    await user.click(connectButton());
    expect(connectButton()).toBeDisabled();

    await user.dblClick(osaka);

    expect(osaka).toHaveAttribute("aria-disabled", "true");
    expect(ipcMock.setActiveProfile).not.toHaveBeenCalled();
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
    expect(ipcMock.restartCore).not.toHaveBeenCalled();
  });

  it("refuses node activation while the backend reports disconnecting", async () => {
    runtimeMock.state.coreState = { ...connectedCoreState, state: "disconnecting" };
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" })]);

    const user = userEvent.setup();
    renderHome();

    const tokyo = await screen.findByRole("option", { name: /Tokyo Edge/ });
    tokyo.focus();
    await user.keyboard("{Enter}");

    expect(tokyo).toHaveAttribute("aria-disabled", "true");
    expect(ipcMock.setActiveProfile).not.toHaveBeenCalled();
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("drops a selection whose node disappeared and connects the active one", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ id: "osaka", remarks: "Osaka Edge" }),
      makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" }),
    ]);

    const user = userEvent.setup();
    const { queryClient } = renderHome();

    await user.click(await screen.findByRole("option", { name: /Tokyo Edge/ }));
    expect(screen.getByRole("option", { name: /Tokyo Edge/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    // The subscription update (or a delete on the Profiles screen) pruned the
    // selected node while the Home screen stayed mounted.
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ id: "osaka", remarks: "Osaka Edge" }),
    ]);
    await act(async () => {
      await queryClient.invalidateQueries({ queryKey: ["profiles"] });
    });

    await waitFor(() =>
      expect(screen.queryByRole("option", { name: /Tokyo Edge/ })).not.toBeInTheDocument(),
    );
    expect(screen.getByRole("option", { name: /Osaka Edge/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    await user.click(connectButton());

    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1));
    expect(ipcMock.setActiveProfile).not.toHaveBeenCalled();
  });

  it("filters the node list by remarks", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" }),
      makeProfile(2, { id: "osaka", remarks: "Osaka Edge" }),
    ]);

    const user = userEvent.setup();
    renderHome();

    await screen.findByRole("option", { name: /Tokyo Edge/ });
    await user.type(screen.getByRole("textbox", { name: "Search nodes…" }), "osaka");

    expect(screen.queryByRole("option", { name: /Tokyo Edge/ })).not.toBeInTheDocument();
    expect(screen.getByRole("option", { name: /Osaka Edge/ })).toBeInTheDocument();
  });

  // The core is fetched on first run, so a connect against a machine without it
  // is the onboarding path rather than an error to toast away.
  it("opens the missing-core recovery modal instead of a toast", async () => {
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" })]);
    ipcMock.connectActiveProfile.mockRejectedValue(
      new ipcMock.IpcCommandError({
        kind: "missingCore",
        message: {
          candidates: [],
          coreType: "singBox",
          downloadUrl: "https://example.test/core",
          message: "sing-box is not installed",
          searchDir: "/cores",
        },
      }),
    );

    const user = userEvent.setup();
    renderHome();
    await screen.findByRole("option", { name: /Tokyo Edge/ });
    await user.click(connectButton());

    await waitFor(() => expect(useModalStore.getState().stack).toHaveLength(1));
    expect(useModalStore.getState().stack[0]).toMatchObject({
      kind: "missingCore",
      missingCore: { coreType: "singBox", message: "sing-box is not installed" },
    });
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });

  it("requests system authorization once and retries a connect that needed it", async () => {
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" })]);
    ipcMock.connectActiveProfile
      .mockRejectedValueOnce(new ipcMock.IpcCommandError({ kind: "sudo", message: "needs root" }))
      .mockResolvedValue(connectedStatus);
    ipcMock.tunRequestElevation.mockResolvedValue({ ...tunStatusResponse, elevationGranted: true });

    const user = userEvent.setup();
    renderHome();
    await screen.findByRole("option", { name: /Tokyo Edge/ });
    await user.click(connectButton());

    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(2));
    expect(ipcMock.tunRequestElevation).toHaveBeenCalledTimes(1);
    expect(useToastStore.getState().toasts).toHaveLength(0);
    expect(useModalStore.getState().stack).toHaveLength(0);
  });

  it("keeps the original failure when the authorization dialog is declined", async () => {
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { id: "tokyo", remarks: "Tokyo Edge" })]);
    ipcMock.connectActiveProfile.mockRejectedValue(
      new ipcMock.IpcCommandError({ kind: "sudo", message: "needs root" }, "sudo helper refused"),
    );
    ipcMock.tunRequestElevation.mockResolvedValue({ ...tunStatusResponse, elevationGranted: false });

    const user = userEvent.setup();
    renderHome();
    await screen.findByRole("option", { name: /Tokyo Edge/ });
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
    runtimeMock.state.coreState = connectedCoreState;
    ipcMock.disconnectCore.mockRejectedValue(disconnectError);
    ipcMock.runtimeStatus.mockResolvedValue(connectedStatus);

    renderHome();

    await user.click(connectButton());

    await waitFor(() => expect(ipcMock.runtimeStatus).toHaveBeenCalledTimes(1));
    expect(runtimeMock.state.setCoreState).toHaveBeenCalledWith({
      activeProfileId: "node-tokyo",
      mainPid: 4242,
      prePid: null,
      runningCoreType: "singBox",
      state: "connected",
    });
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "sudo kill failed",
      title: "Disconnect",
    });
    expect(connectButton()).toBeEnabled();
  });

  it("shows the subscription card empty state with an add path", async () => {
    const user = userEvent.setup();

    renderHome();

    expect(await screen.findByText("No subscription")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add subscription" }));
    expect(await screen.findByRole("dialog", { name: "Subscriptions" })).toBeInTheDocument();
  });

  it("offers the three connection modes and applies system proxy", async () => {
    const user = userEvent.setup();

    renderHome();

    const switcher = screen.getByTestId("home-mode-switcher");
    expect(switcher).toHaveAccessibleName("Connection mode");
    expect(screen.getByRole("button", { name: "Proxy only" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "System proxy" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "VPN" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "System proxy" }));
    await waitFor(() =>
      expect(ipcMock.setConnectionMode).toHaveBeenCalledWith("systemProxy", null),
    );
  });

  it("surfaces the PAC toggle only in system proxy mode and disables it without support", async () => {
    runtimeMock.state.sysProxy = {
      effectiveMode: "forcedChange",
      pacAvailable: false,
      proxy: null,
      requestedMode: "forcedChange",
    };

    renderHome();

    const pac = screen.getByRole("switch", { name: "Smart mode (PAC)" });
    expect(pac).toBeDisabled();
    expect(screen.getByText("Smart proxy is not supported on this platform.")).toBeInTheDocument();
  });

  it("toggles PAC through the unified mode command when supported", async () => {
    const user = userEvent.setup();
    runtimeMock.state.sysProxy = {
      effectiveMode: "forcedChange",
      pacAvailable: true,
      proxy: null,
      requestedMode: "forcedChange",
    };

    renderHome();

    await user.click(screen.getByRole("switch", { name: "Smart mode (PAC)" }));
    await waitFor(() => expect(ipcMock.setConnectionMode).toHaveBeenCalledWith("systemProxy", true));
  });

  it("shows the backend reason and restores controls when mode switching fails", async () => {
    const user = userEvent.setup();
    ipcMock.setConnectionMode.mockRejectedValue(new Error("desktop policy rejected the mode"));

    renderHome();
    const systemProxy = screen.getByRole("button", { name: "System proxy" });
    await user.click(systemProxy);

    await waitFor(() =>
      expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
        description: "desktop policy rejected the mode",
        severity: "error",
        title: "Failed to change connection mode",
      }),
    );
    expect(systemProxy).toBeEnabled();
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
    const systemProxy = screen.getByRole("button", { name: "System proxy" });
    await user.click(systemProxy);
    expect(systemProxy).toBeDisabled();
    await user.click(systemProxy);
    expect(ipcMock.setConnectionMode).toHaveBeenCalledTimes(1);

    resolveMode?.({ ...connectionModeStatus, mode: "systemProxy" });
    await waitFor(() => expect(systemProxy).toBeEnabled());
  });

  it("refuses a mode switch while a connect is still in flight", async () => {
    const user = userEvent.setup();
    // Flipping TUN mid-connect persists the flag but cannot restart a core that
    // is not Connected yet, so the UI would claim VPN over a non-TUN core.
    let resolveConnect: ((status: RuntimeStatusResponse) => void) | undefined;
    ipcMock.connectActiveProfile.mockImplementation(
      () =>
        new Promise<RuntimeStatusResponse>((resolve) => {
          resolveConnect = resolve;
        }),
    );

    renderHome();
    await user.click(screen.getByRole("button", { name: "Connect" }));

    const systemProxy = screen.getByRole("button", { name: "System proxy" });
    expect(systemProxy).toBeDisabled();
    await user.click(systemProxy);
    expect(ipcMock.setConnectionMode).not.toHaveBeenCalled();

    resolveConnect?.(connectedStatus);
    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1));
  });

  it("keeps a committed mode change successful when only the status refresh fails", async () => {
    const user = userEvent.setup();
    // The backend already persisted and emitted its post-commit events, so a
    // failing follow-up read must not be reported as a failed mode change.
    ipcMock.systemProxyStatus.mockRejectedValue(new Error("status read failed"));

    renderHome();
    await user.click(screen.getByRole("button", { name: "System proxy" }));

    await waitFor(() =>
      expect(ipcMock.setConnectionMode).toHaveBeenCalledWith("systemProxy", null),
    );
    await waitFor(() => {
      const titles = useToastStore.getState().toasts.map((toast) => toast.title);
      expect(titles).toContain("Failed to read system proxy status");
      expect(titles).not.toContain("Failed to change connection mode");
    });
  });

  it("requests system authorization on demand before entering VPN mode", async () => {
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

    await user.click(screen.getByRole("button", { name: "VPN" }));

    await waitFor(() => expect(ipcMock.tunRequestElevation).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(ipcMock.setConnectionMode).toHaveBeenCalledWith("vpn", null));
  });

  it("blocks VPN mode when the platform component is missing", async () => {
    const user = userEvent.setup();
    ipcMock.tunStatus.mockResolvedValue({
      ...tunStatusResponse,
      backend: "macosPacketTunnel",
      lastProviderError: "PacketTunnel extension is not bundled in this build",
      nativeComponentReady: false,
      providerState: "missingComponent",
    });

    renderHome();

    await user.click(screen.getByRole("button", { name: "VPN" }));

    await waitFor(() => expect(ipcMock.tunStatus).toHaveBeenCalled());
    expect(ipcMock.setConnectionMode).not.toHaveBeenCalled();
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "PacketTunnel extension is not bundled in this build",
      title: "Failed to enable TUN",
    });
  });

  it("blocks VPN mode when PlugInKit elected a stale provider path", async () => {
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

    await user.click(screen.getByRole("button", { name: "VPN" }));

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

    await user.click(screen.getByRole("button", { name: "VPN" }));

    await waitFor(() => expect(ipcMock.tunRequestElevation).toHaveBeenCalledTimes(1));
    expect(ipcMock.setConnectionMode).not.toHaveBeenCalled();
  });
});

function makeActiveProfile(overrides: Parameters<typeof makeProfileFixture>[1] = {}): ProfileListEntry {
  return { ...makeProfile(0, overrides), isActive: true };
}

function makeProfile(index: number, overrides: Parameters<typeof makeProfileFixture>[1] = {}): ProfileListEntry {
  return makeProfileFixture(index, overrides, false);
}
