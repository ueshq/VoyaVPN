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
  requiresElevation: false,
  restoreOnDisconnect: true,
  elevationGranted: true,
};

vi.mock("@/ipc", () => ({
  listProfiles: vi.fn(() => Promise.resolve([])),
  runtimeStatus: vi.fn(() => Promise.resolve(disconnectedStatus)),
  setSystemProxyMode: vi.fn(),
  setTunEnabled: vi.fn(),
  tunRequestElevation: vi.fn(),
  systemProxyStatus: vi.fn(() => Promise.resolve(sysProxyStatus)),
  tunStatus: vi.fn(() => Promise.resolve(tunStatusResponse)),
  useRuntimeEventStore: runtimeMock.useRuntimeEventStore,
}));

import {
  listProfiles,
  runtimeStatus,
  setSystemProxyMode,
  setTunEnabled,
  tunRequestElevation,
  tunStatus,
} from "@/ipc";
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
    vi.mocked(runtimeStatus).mockResolvedValue(disconnectedStatus);
    vi.mocked(setSystemProxyMode).mockReset();
    vi.mocked(tunStatus).mockResolvedValue(tunStatusResponse);
    vi.mocked(tunRequestElevation).mockReset();
    vi.mocked(setTunEnabled).mockReset();
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

  it("keeps the secondary status and the consolidated TUN control", async () => {
    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 0")).toBeInTheDocument());

    expect(screen.getByTestId("status-bar")).toHaveTextContent("Disconnected");
    // The standalone sudo password control is gone; the single TUN control both
    // shows state and toggles it.
    expect(screen.queryByRole("button", { name: "Sudo" })).toBeNull();
    const tunControl = screen.getByRole("button", { name: "Enable TUN" });
    expect(tunControl).toBeInTheDocument();
    expect(tunControl).toHaveTextContent("TUN off");
  });

  it("does not show the running core in the status bar", async () => {
    const connectedStatus: RuntimeStatusResponse = {
      activeProfileId: "profile-0",
      mainPid: 100,
      prePid: null,
      runningCoreType: 24,
      state: "connected",
    };
    runtimeMock.state.coreState = {
      activeProfileId: "profile-0",
      mainPid: 100,
      prePid: null,
      runningCoreType: 24,
      state: "connected",
    };
    vi.mocked(runtimeStatus).mockResolvedValue(connectedStatus);
    vi.mocked(listProfiles).mockResolvedValue([makeProfile()]);

    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 1")).toBeInTheDocument());
    expect(screen.getByTestId("status-bar")).not.toHaveTextContent("sing-box");
    expect(screen.getByTestId("status-bar")).toHaveTextContent("PID 100");
  });

  it("requests system authorization on demand before switching TUN on", async () => {
    const user = userEvent.setup();
    vi.mocked(tunStatus).mockResolvedValue({
      ...tunStatusResponse,
      requiresElevation: true,
      elevationGranted: false,
    });
    vi.mocked(tunRequestElevation).mockResolvedValue({
      ...tunStatusResponse,
      requiresElevation: true,
      elevationGranted: true,
    });
    vi.mocked(setTunEnabled).mockResolvedValue({ ...tunStatusResponse, enabled: true });

    renderStatusBar();

    const tunControl = await screen.findByRole("button", { name: "Enable TUN" });
    await user.click(tunControl);

    await waitFor(() => expect(tunRequestElevation).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(setTunEnabled).toHaveBeenCalledWith(true));
    expect(runtimeMock.state.setTun).toHaveBeenCalledWith(expect.objectContaining({ enabled: true }));
  });

  it("leaves TUN off when the authorization dialog is cancelled", async () => {
    const user = userEvent.setup();
    vi.mocked(tunStatus).mockResolvedValue({
      ...tunStatusResponse,
      requiresElevation: true,
      elevationGranted: false,
    });
    vi.mocked(tunRequestElevation).mockResolvedValue({
      ...tunStatusResponse,
      requiresElevation: true,
      elevationGranted: false,
    });

    renderStatusBar();

    const tunControl = await screen.findByRole("button", { name: "Enable TUN" });
    await user.click(tunControl);

    await waitFor(() => expect(tunRequestElevation).toHaveBeenCalledTimes(1));
    expect(setTunEnabled).not.toHaveBeenCalled();
    // The mount effect seeds setTun with the initial (off) status; the cancelled
    // toggle must never flip it on.
    expect(runtimeMock.state.setTun).not.toHaveBeenCalledWith(
      expect.objectContaining({ enabled: true }),
    );
  });

  it("surfaces PID, proxy mode, and TUN in the small-window overflow menu", async () => {
    const user = userEvent.setup();
    vi.mocked(setSystemProxyMode).mockResolvedValue(sysProxyStatus);

    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 0")).toBeInTheDocument());

    await user.click(screen.getByLabelText("More controls"));

    const menu = await screen.findByRole("menu");
    expect(within(menu).queryByRole("menuitem", { name: "sing-box" })).toBeNull();
    expect(within(menu).getByText("No PID")).toBeInTheDocument();
    // Proxy mode selection and TUN toggle stay reachable on small windows.
    expect(within(menu).getByRole("menuitemradio", { name: "System proxy cleared" })).toBeInTheDocument();
    expect(within(menu).getByRole("menuitemcheckbox", { name: "TUN" })).toBeInTheDocument();

    await user.click(within(menu).getByRole("menuitemradio", { name: "System proxy forced" }));

    expect(setSystemProxyMode).toHaveBeenCalledWith(1);
  });
});
