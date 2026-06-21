import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  CoreStateEvent,
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
  IpcCommandError: class IpcCommandError extends Error {},
  listProfiles: vi.fn(() => Promise.resolve([])),
  runtimeStatus: vi.fn(() => Promise.resolve(disconnectedStatus)),
  setSystemProxyMode: vi.fn(),
  setTunEnabled: vi.fn(),
  sudoBeginCollection: vi.fn(),
  systemProxyStatus: vi.fn(() => Promise.resolve(sysProxyStatus)),
  tunStatus: vi.fn(() => Promise.resolve(tunStatusResponse)),
  useRuntimeEventStore: runtimeMock.useRuntimeEventStore,
}));

import { listProfiles, setSystemProxyMode } from "@/ipc";

function profilesOfLength(count: number) {
  return Array.from({ length: count }) as unknown as ProfileListItem_Serialize[];
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
    vi.mocked(listProfiles).mockResolvedValue([]);
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

  it("surfaces core info, proxy mode, and TUN in the small-window overflow menu", async () => {
    const user = userEvent.setup();
    vi.mocked(setSystemProxyMode).mockResolvedValue(sysProxyStatus);

    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 0")).toBeInTheDocument());

    await user.click(screen.getByLabelText("More controls"));

    const menu = await screen.findByRole("menu");
    // Core info that md: would otherwise hide is reachable here.
    expect(within(menu).getByText("No core")).toBeInTheDocument();
    expect(within(menu).getByText("No PID")).toBeInTheDocument();
    // Proxy mode selection and TUN toggle stay reachable on small windows.
    expect(within(menu).getByRole("menuitemradio", { name: "System proxy cleared" })).toBeInTheDocument();
    expect(within(menu).getByRole("menuitemcheckbox", { name: "TUN" })).toBeInTheDocument();

    await user.click(within(menu).getByRole("menuitemradio", { name: "System proxy forced" }));

    expect(setSystemProxyMode).toHaveBeenCalledWith(1);
  });
});
