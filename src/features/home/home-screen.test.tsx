import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  CoreStateEvent,
  ProfileItem_Deserialize,
  ProfileListItem_Serialize,
  RuntimeStatusResponse,
  StatisticsSnapshot,
} from "@/ipc/bindings";
import { useModalStore } from "@/stores/modal-store";

import { CONFIG_TYPES } from "@/features/profiles/profile-constants";
import { HomeScreen } from "./home-screen";

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

const ipcMock = vi.hoisted(() => ({
  connectActiveProfile: vi.fn(),
  disconnectCore: vi.fn(),
  listProfiles: vi.fn(),
  restartCore: vi.fn(),
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

vi.mock("@/ipc", () => ({
  connectActiveProfile: ipcMock.connectActiveProfile,
  disconnectCore: ipcMock.disconnectCore,
  IpcCommandError: class IpcCommandError extends Error {},
  listProfiles: ipcMock.listProfiles,
  restartCore: ipcMock.restartCore,
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

describe("HomeScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runtimeMock.state.coreState = null;
    runtimeMock.state.statistics = null;
    ipcMock.connectActiveProfile.mockResolvedValue(connectedStatus);
    ipcMock.disconnectCore.mockResolvedValue(disconnectedStatus);
    ipcMock.restartCore.mockResolvedValue(connectedStatus);
    ipcMock.listProfiles.mockResolvedValue([]);
    useModalStore.setState({ stack: [] });
  });

  afterEach(() => {
    cleanup();
  });

  it("renders the calm unprotected hero by default", () => {
    renderHome();

    expect(screen.getByRole("region", { name: "Connection home" })).toBeInTheDocument();
    expect(screen.getByText("Not protected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Connect" })).toBeEnabled();
    expect(screen.getByText("No active node")).toBeInTheDocument();
  });

  it("lights up the protected state and surfaces node and core", () => {
    runtimeMock.state.coreState = {
      activeProfileId: "node-tokyo",
      mainPid: 4242,
      prePid: null,
      runningCoreType: 24,
      state: "connected",
    };

    renderHome();

    expect(screen.getByText("Protected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Disconnect" })).toBeInTheDocument();
    expect(screen.getByText("node-tokyo")).toBeInTheDocument();
    expect(screen.getByText("sing-box")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restart" })).toBeInTheDocument();
  });

  it("resolves the active node name from the profiles cache", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeProfile(1, { IndexId: "node-tokyo", Remarks: "Other Node" }),
      makeActiveProfile({ IndexId: "node-osaka", Remarks: "Osaka Edge" }),
    ]);

    renderHome();

    // Falls back to the running id until the query resolves, then shows Remarks.
    expect(await screen.findByText("Osaka Edge")).toBeInTheDocument();
  });

  it("invokes the connect action from the primary key", async () => {
    const user = userEvent.setup();

    renderHome();

    await user.click(screen.getByRole("button", { name: "Connect" }));

    expect(ipcMock.connectActiveProfile).toHaveBeenCalledTimes(1);
    expect(ipcMock.disconnectCore).not.toHaveBeenCalled();
  });

  it("opens the node picker from the clickable Node tile", async () => {
    const user = userEvent.setup();

    renderHome();

    await user.click(screen.getByRole("button", { name: "Change node" }));

    expect(useModalStore.getState().stack.at(-1)?.kind).toBe("nodePicker");
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
