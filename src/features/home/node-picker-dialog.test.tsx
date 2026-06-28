import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { Dialog } from "@/components/ui/dialog";
import type {
  CoreStateEvent,
  ProfileItem_Deserialize,
  ProfileListItem_Serialize,
  RuntimeStatusResponse,
} from "@/ipc/bindings";
import { useModalStore } from "@/stores/modal-store";

import { CONFIG_TYPES } from "@/features/profiles/profile-constants";
import { NodePickerDialog } from "./node-picker-dialog";

type RuntimeState = {
  coreState: CoreStateEvent | null;
  setCoreState: (state: CoreStateEvent) => void;
};

const runtimeMock = vi.hoisted(() => {
  const state: RuntimeState = { coreState: null, setCoreState: vi.fn() };
  const useRuntimeEventStore = (selector: (value: RuntimeState) => unknown) => selector(state);

  return { state, useRuntimeEventStore };
});

const ipcMock = vi.hoisted(() => ({
  listProfiles: vi.fn(),
  restartCore: vi.fn(),
  setActiveProfile: vi.fn(),
}));

const connectedStatus: RuntimeStatusResponse = {
  activeProfileId: "node-tokyo",
  mainPid: 4242,
  prePid: null,
  runningCoreType: 2,
  state: "connected",
};

vi.mock("@/ipc", () => ({
  IpcCommandError: class IpcCommandError extends Error {},
  listProfiles: ipcMock.listProfiles,
  restartCore: ipcMock.restartCore,
  setActiveProfile: ipcMock.setActiveProfile,
  useRuntimeEventStore: runtimeMock.useRuntimeEventStore,
}));

function renderPicker() {
  const queryClient = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { gcTime: 0, retry: false } },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <Dialog onOpenChange={() => {}} open>
        <NodePickerDialog />
      </Dialog>
    </QueryClientProvider>,
  );
}

describe("NodePickerDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runtimeMock.state.coreState = null;
    ipcMock.listProfiles.mockResolvedValue([]);
    ipcMock.restartCore.mockResolvedValue(connectedStatus);
    ipcMock.setActiveProfile.mockResolvedValue(makeProfile(0));
    // Pretend the picker is the open modal so closeTopModal() is observable.
    useModalStore.setState({ stack: [{ id: "node-picker", kind: "nodePicker" }] });
  });

  afterEach(() => {
    cleanup();
  });

  it("lists nodes with the active node pinned to the top", async () => {
    const active = makeProfile(2, { IndexId: "osaka", Remarks: "Osaka Edge" });
    active.isActive = true;
    active.profileEx = { ...active.profileEx, Delay: 42 };

    ipcMock.listProfiles.mockResolvedValue([
      makeProfile(1, { IndexId: "tokyo", Remarks: "Tokyo Edge" }),
      active,
    ]);

    renderPicker();

    expect(await screen.findByRole("button", { name: /Osaka Edge/ })).toBeInTheDocument();
    expect(screen.getByText("42 ms")).toBeInTheDocument();

    const nodeButtons = screen
      .getAllByRole("button")
      .filter((button) => /Edge/.test(button.textContent ?? ""));
    expect(nodeButtons[0]).toHaveTextContent("Osaka Edge");
  });

  it("filters the list by remarks or address", async () => {
    ipcMock.listProfiles.mockResolvedValue([
      makeProfile(1, { Address: "tokyo.example.test", IndexId: "tokyo", Remarks: "Tokyo Edge" }),
      makeProfile(2, { Address: "osaka.example.test", IndexId: "osaka", Remarks: "Osaka Edge" }),
    ]);

    const user = userEvent.setup();
    renderPicker();

    await screen.findByRole("button", { name: /Tokyo Edge/ });
    await user.type(screen.getByRole("textbox", { name: "Search nodes…" }), "osaka");

    expect(screen.queryByRole("button", { name: /Tokyo Edge/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Osaka Edge/ })).toBeInTheDocument();
  });

  it("sets the node active without reconnecting while disconnected", async () => {
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { IndexId: "tokyo", Remarks: "Tokyo Edge" })]);

    const user = userEvent.setup();
    renderPicker();

    await user.click(await screen.findByRole("button", { name: /Tokyo Edge/ }));

    expect(ipcMock.setActiveProfile).toHaveBeenCalledWith("tokyo");
    expect(ipcMock.restartCore).not.toHaveBeenCalled();
    await waitFor(() => expect(useModalStore.getState().stack).toHaveLength(0));
  });

  it("restarts the core to apply the switch while connected", async () => {
    runtimeMock.state.coreState = {
      activeProfileId: "node-old",
      mainPid: 1,
      prePid: null,
      runningCoreType: 2,
      state: "connected",
    };
    ipcMock.listProfiles.mockResolvedValue([makeProfile(1, { IndexId: "tokyo", Remarks: "Tokyo Edge" })]);

    const user = userEvent.setup();
    renderPicker();

    await user.click(await screen.findByRole("button", { name: /Tokyo Edge/ }));

    expect(ipcMock.setActiveProfile).toHaveBeenCalledWith("tokyo");
    await waitFor(() => expect(ipcMock.restartCore).toHaveBeenCalledTimes(1));
    expect(runtimeMock.state.setCoreState).toHaveBeenCalled();
  });
});

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
