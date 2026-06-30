import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  CoreStateEvent,
  ProfileItem_Deserialize,
  ProfileListItem_Serialize,
  RuntimeStatusResponse,
  SysProxyChanged,
  SystemProxyStatusResponse,
  TunChanged,
  TunStatus,
} from "@/ipc/bindings";
import { useToastStore } from "@/stores/toast-store";

import { CONFIG_TYPES } from "@/features/profiles/profile-constants";
import { HomeScreen } from "./home-screen";

type RuntimeState = {
  coreState: CoreStateEvent | null;
  setCoreState: (state: CoreStateEvent) => void;
  sysProxy: SysProxyChanged | null;
  setSysProxy: (state: SysProxyChanged) => void;
  tun: TunChanged | null;
  setTun: (state: TunChanged) => void;
};

const runtimeMock = vi.hoisted(() => {
  const state: RuntimeState = {
    coreState: null,
    setCoreState: vi.fn(),
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

const ipcMock = vi.hoisted(() => ({
  connectActiveProfile: vi.fn(),
  disconnectCore: vi.fn(),
  listProfiles: vi.fn(),
  restartCore: vi.fn(),
  runtimeStatus: vi.fn(),
  setActiveProfile: vi.fn(),
  setSystemProxyMode: vi.fn(),
  setTunEnabled: vi.fn(),
  systemProxyStatus: vi.fn(),
  tunRequestElevation: vi.fn(),
  tunStatus: vi.fn(),
}));

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
  runningCoreType: 24,
  state: "connected",
};

const sysProxyStatus: SystemProxyStatusResponse = {
  effectiveMode: 0,
  exceptions: "",
  pacAvailable: false,
  pacUrl: null,
  proxy: null,
  requestedMode: 0,
};

const tunStatusResponse: TunStatus = {
  allowEnableTun: true,
  enabled: false,
  preflight: {
    notes: [],
    platform: "macos",
    routeRestoreNote: "",
    state: "ready",
    windowsCleanupDevices: [],
  },
  requiresElevation: false,
  restoreOnDisconnect: true,
  elevationGranted: true,
};

vi.mock("@/ipc", () => ({
  connectActiveProfile: ipcMock.connectActiveProfile,
  disconnectCore: ipcMock.disconnectCore,
  IpcCommandError: class IpcCommandError extends Error {},
  listProfiles: ipcMock.listProfiles,
  restartCore: ipcMock.restartCore,
  runtimeStatus: ipcMock.runtimeStatus,
  setActiveProfile: ipcMock.setActiveProfile,
  setSystemProxyMode: ipcMock.setSystemProxyMode,
  setTunEnabled: ipcMock.setTunEnabled,
  systemProxyStatus: ipcMock.systemProxyStatus,
  tunRequestElevation: ipcMock.tunRequestElevation,
  tunStatus: ipcMock.tunStatus,
  useRuntimeEventStore: runtimeMock.useRuntimeEventStore,
}));

function renderHome() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { gcTime: 0, retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <HomeScreen />
    </QueryClientProvider>,
  );
}

const connectedCoreState: CoreStateEvent = {
  activeProfileId: "node-tokyo",
  mainPid: 4242,
  prePid: null,
  runningCoreType: 24,
  state: "connected",
};

describe("HomeScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runtimeMock.state.coreState = null;
    runtimeMock.state.sysProxy = null;
    runtimeMock.state.tun = null;
    ipcMock.connectActiveProfile.mockResolvedValue(connectedStatus);
    ipcMock.disconnectCore.mockResolvedValue(disconnectedStatus);
    ipcMock.restartCore.mockResolvedValue(connectedStatus);
    ipcMock.runtimeStatus.mockResolvedValue(disconnectedStatus);
    ipcMock.listProfiles.mockResolvedValue([]);
    ipcMock.setActiveProfile.mockResolvedValue(makeProfile(0));
    ipcMock.setSystemProxyMode.mockResolvedValue(sysProxyStatus);
    ipcMock.setTunEnabled.mockResolvedValue(tunStatusResponse);
    ipcMock.systemProxyStatus.mockResolvedValue(sysProxyStatus);
    ipcMock.tunRequestElevation.mockResolvedValue(tunStatusResponse);
    ipcMock.tunStatus.mockResolvedValue(tunStatusResponse);
    useToastStore.setState({ toasts: [] });
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the calm unprotected hero with an empty node list by default", async () => {
    renderHome();

    expect(screen.getByRole("region", { name: "Connection home" })).toBeInTheDocument();
    expect(screen.getByText("Not protected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Connect" })).toBeEnabled();
    expect(await screen.findByText("No nodes available")).toBeInTheDocument();
  });

  it("lights up the protected state and marks the running node in the list", async () => {
    runtimeMock.state.coreState = connectedCoreState;
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ IndexId: "node-tokyo", Remarks: "Tokyo Edge" }),
    ]);

    renderHome();

    expect(screen.getByText("Protected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Disconnect" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restart" })).toBeInTheDocument();

    const row = await screen.findByRole("option", { name: /Tokyo Edge/ });
    // Blue selection is seeded to the active node; the green "live" dot marks the
    // node that is actually running.
    expect(row).toHaveAttribute("aria-selected", "true");
    expect(row.querySelector(".bg-connected")).not.toBeNull();
  });

  it("selects a node locally on single click without touching the backend", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ IndexId: "osaka", Remarks: "Osaka Edge" }),
      makeProfile(1, { IndexId: "tokyo", Remarks: "Tokyo Edge" }),
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
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { IndexId: "tokyo", Remarks: "Tokyo Edge" })]);

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
      runningCoreType: 24,
      state: "connected",
    };
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { IndexId: "tokyo", Remarks: "Tokyo Edge" })]);

    const user = userEvent.setup();
    renderHome();

    await user.dblClick(await screen.findByRole("option", { name: /Tokyo Edge/ }));

    expect(ipcMock.setActiveProfile).toHaveBeenCalledWith("tokyo");
    await waitFor(() => expect(ipcMock.restartCore).toHaveBeenCalledTimes(1));
    expect(ipcMock.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("activates the focused node on Enter", async () => {
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { IndexId: "tokyo", Remarks: "Tokyo Edge" })]);

    const user = userEvent.setup();
    renderHome();

    const tokyo = await screen.findByRole("option", { name: /Tokyo Edge/ });
    tokyo.focus();
    await user.keyboard("{Enter}");

    expect(ipcMock.setActiveProfile).toHaveBeenCalledWith("tokyo");
    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1));
  });

  it("invokes the connect action from the primary key", async () => {
    const user = userEvent.setup();

    renderHome();

    await user.click(screen.getByRole("button", { name: "Connect" }));

    expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1);
    expect(ipcMock.disconnectCore).not.toHaveBeenCalled();
  });

  it("connects to the locally selected node, switching the active profile first", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ IndexId: "osaka", Remarks: "Osaka Edge" }),
      makeProfile(1, { IndexId: "tokyo", Remarks: "Tokyo Edge" }),
    ]);

    const user = userEvent.setup();
    renderHome();

    await user.click(await screen.findByRole("option", { name: /Tokyo Edge/ }));
    await user.click(screen.getByRole("button", { name: "Connect" }));

    expect(ipcMock.setActiveProfile).toHaveBeenCalledWith("tokyo");
    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1));
    // The active profile is switched before connect so the tunnel uses it.
    expect(ipcMock.setActiveProfile.mock.invocationCallOrder[0]).toBeLessThan(
      ipcMock.connectActiveProfile.mock.invocationCallOrder[0],
    );
  });

  it("connects directly when the selection already matches the active node", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeActiveProfile({ IndexId: "osaka", Remarks: "Osaka Edge" }),
    ]);

    const user = userEvent.setup();
    renderHome();

    await screen.findByRole("option", { name: /Osaka Edge/ });
    await user.click(screen.getByRole("button", { name: "Connect" }));

    await waitFor(() => expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1));
    expect(ipcMock.setActiveProfile).not.toHaveBeenCalled();
  });

  it("filters the node list by remarks", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeProfile(1, { IndexId: "tokyo", Remarks: "Tokyo Edge" }),
      makeProfile(2, { IndexId: "osaka", Remarks: "Osaka Edge" }),
    ]);

    const user = userEvent.setup();
    renderHome();

    await screen.findByRole("option", { name: /Tokyo Edge/ });
    await user.type(screen.getByRole("textbox", { name: "Search nodes…" }), "osaka");

    expect(screen.queryByRole("option", { name: /Tokyo Edge/ })).not.toBeInTheDocument();
    expect(screen.getByRole("option", { name: /Osaka Edge/ })).toBeInTheDocument();
  });

  it("refreshes runtime state and surfaces errors when disconnect fails", async () => {
    const user = userEvent.setup();
    const disconnectError = new Error("sudo kill failed");
    runtimeMock.state.coreState = connectedCoreState;
    ipcMock.disconnectCore.mockRejectedValue(disconnectError);
    ipcMock.runtimeStatus.mockResolvedValue(connectedStatus);

    renderHome();

    await user.click(screen.getByRole("button", { name: "Disconnect" }));

    await waitFor(() => expect(ipcMock.runtimeStatus).toHaveBeenCalledTimes(1));
    expect(runtimeMock.state.setCoreState).toHaveBeenCalledWith({
      activeProfileId: "node-tokyo",
      mainPid: 4242,
      prePid: null,
      runningCoreType: 24,
      state: "connected",
    });
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "sudo kill failed",
      title: "Disconnect",
    });
    expect(screen.getByRole("button", { name: "Disconnect" })).toBeEnabled();
  });

  it("offers the three system-proxy modes and applies the picked one", async () => {
    const user = userEvent.setup();

    renderHome();

    expect(screen.getByRole("button", { name: "Direct" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Smart" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Global" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Global" }));
    expect(ipcMock.setSystemProxyMode).toHaveBeenCalledWith(1);

    await user.click(screen.getByRole("button", { name: "Smart" }));
    expect(ipcMock.setSystemProxyMode).toHaveBeenCalledWith(3);
  });

  it("requests system authorization on demand before switching TUN on", async () => {
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
    ipcMock.setTunEnabled.mockResolvedValue({ ...tunStatusResponse, enabled: true });

    renderHome();

    await user.click(screen.getByRole("switch", { name: "TUN" }));

    await waitFor(() => expect(ipcMock.tunRequestElevation).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(ipcMock.setTunEnabled).toHaveBeenCalledWith(true));
    expect(runtimeMock.state.setTun).toHaveBeenCalledWith(expect.objectContaining({ enabled: true }));
  });

  it("leaves TUN off when the authorization dialog is cancelled", async () => {
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

    await user.click(screen.getByRole("switch", { name: "TUN" }));

    await waitFor(() => expect(ipcMock.tunRequestElevation).toHaveBeenCalledTimes(1));
    expect(ipcMock.setTunEnabled).not.toHaveBeenCalled();
    expect(runtimeMock.state.setTun).not.toHaveBeenCalledWith(expect.objectContaining({ enabled: true }));
  });
});

function makeActiveProfile(overrides: ProfileItem_Deserialize = {}): ProfileListItem_Serialize {
  return { ...makeProfile(0, overrides), isActive: true };
}

function makeProfile(index: number, overrides: ProfileItem_Deserialize = {}): ProfileListItem_Serialize {
  const indexId = overrides.IndexId ?? `profile-${index}`;

  return {
    isActive: false,
    profile: {
      Address: `node-${index}.example.test`,
      AllowInsecure: "false",
      Alpn: "",
      Cert: "",
      CertSha: "",
      ConfigType: CONFIG_TYPES.VMess,
      ConfigVersion: 4,
      DisplayLog: true,
      EchConfigList: "",
      Finalmask: "",
      Fingerprint: "",
      IndexId: indexId,
      IsSub: false,
      Mldsa65Verify: "",
      MuxEnabled: false,
      Network: "tcp",
      Password: `uuid-${index}`,
      Port: 443,
      ProtocolExtra: {},
      PublicKey: "",
      Remarks: `Server ${index}`,
      ShortId: "",
      Sni: "",
      SpiderX: "",
      StreamSecurity: "",
      Subid: "",
      TransportExtra: {},
      Username: "",
      ...overrides,
    },
    profileEx: {
      Delay: 0,
      IndexId: indexId,
      IpInfo: null,
      Message: null,
      Sort: index * 10,
      Speed: null,
    },
    serverStat: {
      DateNow: 1,
      IndexId: indexId,
      TodayDown: 0,
      TodayUp: 0,
      TotalDown: 0,
      TotalUp: 0,
    },
  };
}
