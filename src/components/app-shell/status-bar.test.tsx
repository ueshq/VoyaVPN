import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  CoreStateEvent,
  ProfileItem_Deserialize,
  ProfileListItem_Serialize,
  RuntimeStatusResponse,
  StatisticsSnapshot,
} from "@/ipc/bindings";

import { StatusBar } from "./status-bar";

type RuntimeState = {
  coreState: CoreStateEvent | null;
  setCoreState: (state: CoreStateEvent) => void;
  statistics: StatisticsSnapshot | null;
};

const runtimeMock = vi.hoisted(() => {
  const state: RuntimeState = {
    coreState: null,
    setCoreState: vi.fn(),
    statistics: null,
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

vi.mock("@/ipc", () => ({
  listProfiles: vi.fn(() => Promise.resolve([])),
  runtimeStatus: vi.fn(() => Promise.resolve(disconnectedStatus)),
  useRuntimeEventStore: runtimeMock.useRuntimeEventStore,
}));

import { listProfiles, runtimeStatus } from "@/ipc";
import { useShellStore } from "@/stores/shell-store";

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
    vi.mocked(runtimeMock.state.setCoreState).mockClear();
    useShellStore.setState({ activeTab: "home" });
    vi.mocked(listProfiles).mockResolvedValue([]);
    vi.mocked(runtimeStatus).mockResolvedValue(disconnectedStatus);
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

  it("drops the runtime keys now owned by the hero", async () => {
    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 0")).toBeInTheDocument());

    expect(screen.queryByRole("button", { name: "Connect" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Disconnect" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Restart" })).toBeNull();
  });

  it("no longer hosts the proxy mode or TUN controls (moved to the home hero)", async () => {
    renderStatusBar();

    await waitFor(() => expect(screen.getByText("Profiles: 0")).toBeInTheDocument());

    expect(screen.getByTestId("status-bar")).toHaveTextContent("Disconnected");
    expect(screen.queryByRole("group", { name: "System proxy mode" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Direct" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Global" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Enable TUN" })).toBeNull();
    expect(screen.queryByRole("switch")).toBeNull();
    expect(screen.queryByLabelText("More controls")).toBeNull();
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
});
