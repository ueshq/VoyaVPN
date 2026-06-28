import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  CoreStateEvent,
  ProfileItem_Deserialize,
  ProfileListItem_Serialize,
  RuntimeStatusResponse,
  StatisticsSnapshot,
  SysProxyChanged,
  SystemProxyStatusResponse,
  TunChanged,
  TunStatus,
} from "@/ipc/bindings";

import { StatusBar } from "./status-bar";

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

const disconnectedStatus: RuntimeStatusResponse = {
  activeProfileId: null,
  mainPid: null,
  prePid: null,
  runningCoreType: null,
  state: "disconnected",
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
  requiresSudoPassword: false,
  restoreOnDisconnect: true,
  sudoPasswordPresent: true,
};

vi.mock("@/ipc", () => ({
  IpcCommandError: class IpcCommandError extends Error {
    readonly appError: unknown;

    constructor(appError: unknown) {
      super(typeof appError === "object" && appError ? JSON.stringify(appError) : "IPC command failed");
      this.appError = appError;
    }
  },
  listProfiles: vi.fn(() => Promise.resolve([])),
  restartCore: vi.fn(),
  runtimeStatus: vi.fn(() => Promise.resolve(disconnectedStatus)),
  saveProfile: vi.fn(),
  setSystemProxyMode: vi.fn(),
  setTunEnabled: vi.fn(),
  sudoBeginCollection: vi.fn(),
  systemProxyStatus: vi.fn(() => Promise.resolve(sysProxyStatus)),
  tunStatus: vi.fn(() => Promise.resolve(tunStatusResponse)),
  useRuntimeEventStore: runtimeMock.useRuntimeEventStore,
}));

import { IpcCommandError, listProfiles, restartCore, runtimeStatus, saveProfile, setSystemProxyMode } from "@/ipc";
import { useModalStore } from "@/stores/modal-store";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";

function profilesOfLength(count: number) {
  return Array.from({ length: count }) as unknown as ProfileListItem_Serialize[];
}

function makeProfile(
  index = 0,
  overrides: ProfileItem_Deserialize = {},
  isActive = true,
): ProfileListItem_Serialize {
  const indexId = overrides.IndexId ?? `profile-${index}`;

  return {
    isActive,
    profile: {
      Address: `node-${index}.example.test`,
      AllowInsecure: "false",
      Alpn: "",
      Cert: "",
      CertSha: "",
      ConfigType: 1,
      ConfigVersion: 4,
      CoreType: null,
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
      Sort: index,
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

function renderStatusBar() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { gcTime: 0, retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <StatusBar />
    </QueryClientProvider>,
  );
}

describe("StatusBar", () => {
  beforeEach(() => {
    runtimeMock.state.coreState = null;
    runtimeMock.state.statistics = null;
    runtimeMock.state.sysProxy = null;
    runtimeMock.state.tun = null;
    vi.mocked(runtimeMock.state.setCoreState).mockClear();
    vi.mocked(runtimeMock.state.setSysProxy).mockClear();
    vi.mocked(runtimeMock.state.setTun).mockClear();
    useShellStore.setState({ activeTab: "home" });
    useModalStore.setState({ stack: [] });
    useToastStore.setState({ toasts: [] });
    vi.mocked(listProfiles).mockResolvedValue([]);
    vi.mocked(restartCore).mockReset();
    vi.mocked(runtimeStatus).mockResolvedValue(disconnectedStatus);
    vi.mocked(saveProfile).mockImplementation(async (profile) =>
      makeProfile(0, profile, true),
    );
    vi.mocked(setSystemProxyMode).mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the real profile count from the profiles query", async () => {
    vi.mocked(listProfiles).mockResolvedValue(profilesOfLength(3));

    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 3")).toBeInTheDocument());
    expect(listProfiles).toHaveBeenCalled();
  });

  it("reflects an empty profile list as zero rather than a hardcoded fallback", async () => {
    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 0")).toBeInTheDocument());
  });

  it("shows the default page route from the shell store", () => {
    renderStatusBar();

    expect(screen.getByText("Route: /home")).toBeInTheDocument();
  });

  it("shows the selected page route from the shell store", () => {
    useShellStore.setState({ activeTab: "routing" });

    renderStatusBar();

    expect(screen.getByText("Route: /routing")).toBeInTheDocument();
  });

  it("drops the connect, disconnect, and restart keys now owned by the hero", async () => {
    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 0")).toBeInTheDocument());

    expect(screen.queryByRole("button", { name: "Connect" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Disconnect" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Restart" })).toBeNull();
  });

  it("keeps the secondary status, sudo, and TUN controls", async () => {
    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 0")).toBeInTheDocument());

    expect(screen.getByTestId("status-bar")).toHaveTextContent("Disconnected");
    expect(screen.getByRole("button", { name: "Sudo" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Enable TUN" })).toBeInTheDocument();
  });

  it("shows the effective active profile core while disconnected", async () => {
    vi.mocked(listProfiles).mockResolvedValue([makeProfile(0, { CoreType: null })]);

    renderStatusBar();

    await waitFor(() => expect(screen.getByLabelText("Switch core")).toHaveTextContent("Xray"));
    expect(screen.queryByText("No active node")).not.toBeInTheDocument();
  });

  it("saves the selected core on the active profile without restarting while disconnected", async () => {
    const user = userEvent.setup();
    vi.mocked(listProfiles).mockResolvedValue([makeProfile(0, { CoreType: null })]);

    renderStatusBar();

    await waitFor(() => expect(screen.getByLabelText("Switch core")).toHaveTextContent("Xray"));
    await user.click(screen.getByLabelText("Switch core"));
    await user.click(await screen.findByRole("menuitemradio", { name: "sing-box" }));

    await waitFor(() =>
      expect(saveProfile).toHaveBeenCalledWith(expect.objectContaining({ CoreType: 24 })),
    );
    expect(restartCore).not.toHaveBeenCalled();
  });

  it("saves the default core selection as null", async () => {
    const user = userEvent.setup();
    vi.mocked(listProfiles).mockResolvedValue([makeProfile(0, { CoreType: 24 })]);

    renderStatusBar();

    await waitFor(() => expect(screen.getByLabelText("Switch core")).toHaveTextContent("sing-box"));
    await user.click(screen.getByLabelText("Switch core"));
    await user.click(await screen.findByRole("menuitemradio", { name: "Default" }));

    await waitFor(() =>
      expect(saveProfile).toHaveBeenCalledWith(expect.objectContaining({ CoreType: null })),
    );
  });

  it("restarts the connected core after saving a new active profile core", async () => {
    const user = userEvent.setup();
    const connectedStatus: RuntimeStatusResponse = {
      activeProfileId: "profile-0",
      mainPid: 100,
      prePid: null,
      runningCoreType: 2,
      state: "connected",
    };
    const switchedStatus: RuntimeStatusResponse = {
      ...connectedStatus,
      runningCoreType: 24,
    };

    runtimeMock.state.coreState = {
      activeProfileId: "profile-0",
      mainPid: 100,
      prePid: null,
      runningCoreType: 2,
      state: "connected",
    };
    vi.mocked(runtimeStatus).mockResolvedValue(connectedStatus);
    vi.mocked(listProfiles).mockResolvedValue([makeProfile(0, { CoreType: 2 })]);
    vi.mocked(restartCore).mockResolvedValue(switchedStatus);

    renderStatusBar();

    await waitFor(() => expect(screen.getByLabelText("Switch core")).toHaveTextContent("Xray"));
    await user.click(screen.getByLabelText("Switch core"));
    await user.click(await screen.findByRole("menuitemradio", { name: "sing-box" }));

    await waitFor(() => expect(restartCore).toHaveBeenCalledTimes(1));
    expect(vi.mocked(saveProfile).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(restartCore).mock.invocationCallOrder[0],
    );
    expect(runtimeMock.state.setCoreState).toHaveBeenCalledWith(
      expect.objectContaining({ runningCoreType: 24, state: "connected" }),
    );
  });

  it("opens the missing-core modal when restart reports a missing core", async () => {
    const user = userEvent.setup();

    runtimeMock.state.coreState = {
      activeProfileId: "profile-0",
      mainPid: 100,
      prePid: null,
      runningCoreType: 2,
      state: "connected",
    };
    vi.mocked(listProfiles).mockResolvedValue([makeProfile(0, { CoreType: 2 })]);
    vi.mocked(restartCore).mockRejectedValue(
      new IpcCommandError({
        kind: "missingCore",
        message: { coreType: 24, message: "missing sing-box" },
      } as never),
    );

    renderStatusBar();

    await waitFor(() => expect(screen.getByLabelText("Switch core")).toHaveTextContent("Xray"));
    await user.click(screen.getByLabelText("Switch core"));
    await user.click(await screen.findByRole("menuitemradio", { name: "sing-box" }));

    await waitFor(() =>
      expect(useModalStore.getState().stack.at(-1)).toMatchObject({
        kind: "missingCore",
        missingCore: { coreType: 24 },
      }),
    );
  });

  it("opens the sudo modal when restart needs credentials", async () => {
    const user = userEvent.setup();

    runtimeMock.state.coreState = {
      activeProfileId: "profile-0",
      mainPid: 100,
      prePid: null,
      runningCoreType: 2,
      state: "connected",
    };
    vi.mocked(listProfiles).mockResolvedValue([makeProfile(0, { CoreType: 2 })]);
    vi.mocked(restartCore).mockRejectedValue(
      new IpcCommandError({ kind: "sudo", message: "sudo password required" } as never),
    );

    renderStatusBar();

    await waitFor(() => expect(screen.getByLabelText("Switch core")).toHaveTextContent("Xray"));
    await user.click(screen.getByLabelText("Switch core"));
    await user.click(await screen.findByRole("menuitemradio", { name: "sing-box" }));

    await waitFor(() => expect(useModalStore.getState().stack.at(-1)?.kind).toBe("sudo"));
  });

  it("surfaces core info, proxy mode, and TUN in the small-window overflow menu", async () => {
    const user = userEvent.setup();
    vi.mocked(listProfiles).mockResolvedValue([makeProfile(0, { CoreType: null })]);
    vi.mocked(setSystemProxyMode).mockResolvedValue(sysProxyStatus);

    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 0")).toBeInTheDocument());

    await user.click(screen.getByLabelText("More controls"));

    const menu = await screen.findByRole("menu");
    // Core info that md: would otherwise hide is reachable here.
    expect(within(menu).getByRole("menuitem", { name: "Xray" })).toBeInTheDocument();
    expect(within(menu).getByText("No PID")).toBeInTheDocument();
    // Proxy mode selection and TUN toggle stay reachable on small windows.
    expect(within(menu).getByRole("menuitemradio", { name: "System proxy cleared" })).toBeInTheDocument();
    expect(within(menu).getByRole("menuitemcheckbox", { name: "TUN" })).toBeInTheDocument();

    await user.click(within(menu).getByRole("menuitemradio", { name: "System proxy forced" }));

    expect(setSystemProxyMode).toHaveBeenCalledWith(1);
  });
});
