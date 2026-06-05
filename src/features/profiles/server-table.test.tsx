import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { vi } from "vitest";

import type { ProfileItem_Deserialize, ProfileListItem_Serialize } from "@/ipc/bindings";

import { CONFIG_TYPES, MOVE_ACTIONS, PROFILE_PROTOCOLS } from "./profile-constants";
import { ProfilesScreen } from "./server-table";
import { applyLiveUpdates } from "./server-table-live-updates";

const ipcMocks = vi.hoisted(() => ({
  copyProfiles: vi.fn(),
  dedupeProfiles: vi.fn(),
  deleteSubscriptions: vi.fn(),
  deleteProfiles: vi.fn(),
  importProfilesFromText: vi.fn(),
  listGroupChildCandidates: vi.fn(),
  listProfiles: vi.fn(),
  listSubscriptions: vi.fn(),
  moveProfile: vi.fn(),
  previewGroupProfile: vi.fn(),
  saveGroupProfile: vi.fn(),
  saveProfile: vi.fn(),
  saveSubscription: vi.fn(),
  setActiveProfile: vi.fn(),
  sortProfiles: vi.fn(),
  updateSubscriptions: vi.fn(),
  validateGroupProfile: vi.fn(),
  useRuntimeEventStore: Object.assign(
    (selector: (state: unknown) => unknown) =>
      selector({
        clearLogs: vi.fn(),
        coreState: null,
        lastTransientEvent: null,
        logLines: [],
        pushTransientEvent: vi.fn(),
        serverStatsByProfileId: {},
        setCoreState: vi.fn(),
        setSysProxy: vi.fn(),
        speedtestResultsByProfileId: {},
        statistics: null,
        sysProxy: null,
        tun: null,
      }),
    { getState: vi.fn() },
  ),
}));

vi.mock("@/ipc", () => ipcMocks);

function renderProfiles() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
    },
  });

  return render(
    <QueryClientProvider client={queryClient}>
      <ProfilesScreen />
    </QueryClientProvider>,
  );
}

async function selectComboboxOption(label: string, optionLabel: string) {
  const user = userEvent.setup();

  await user.click(screen.getByRole("combobox", { name: label }));
  const listbox = await screen.findByRole("listbox");
  await user.click(within(listbox).getByRole("option", { name: new RegExp(`^${escapeRegExp(optionLabel)}`) }));
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

describe("ProfilesScreen", () => {
  beforeEach(() => {
    Object.values(ipcMocks).forEach((mock) => {
      if ("mockReset" in mock) {
        mock.mockReset();
      }
    });
    ipcMocks.copyProfiles.mockResolvedValue([]);
    ipcMocks.dedupeProfiles.mockResolvedValue({ kept: 0, removedIndexIds: [], total: 0 });
    ipcMocks.deleteSubscriptions.mockResolvedValue(1);
    ipcMocks.deleteProfiles.mockResolvedValue(1);
    ipcMocks.importProfilesFromText.mockResolvedValue({ imported: 1, importedIndexIds: ["profile-new"], removedExisting: 0, skipped: 0, subid: null });
    ipcMocks.listGroupChildCandidates.mockResolvedValue([]);
    ipcMocks.listSubscriptions.mockResolvedValue([]);
    ipcMocks.moveProfile.mockResolvedValue([]);
    ipcMocks.previewGroupProfile.mockResolvedValue({
      validation: { childIndexIds: [], errors: [], normalizedChildItems: "", valid: true, warnings: [] },
      xrayRoutes: [],
      xrayBalancers: [],
      xrayObservatorySelectors: [],
      xrayBurstObservatorySelectors: [],
      singboxRoutes: [],
    });
    ipcMocks.saveGroupProfile.mockImplementation(async (profile: ProfileItem_Deserialize) => makeProfile(100, profile));
    ipcMocks.saveProfile.mockImplementation(async (profile: ProfileItem_Deserialize) => makeProfile(99, profile));
    ipcMocks.saveSubscription.mockResolvedValue(makeSubscription());
    ipcMocks.setActiveProfile.mockImplementation(async (indexId: string) => makeProfile(0, { IndexId: indexId }));
    ipcMocks.sortProfiles.mockResolvedValue([]);
    ipcMocks.updateSubscriptions.mockResolvedValue({ imported: 0, messages: [], removedExisting: 0, skipped: 0, updated: 0 });
    ipcMocks.validateGroupProfile.mockResolvedValue({ childIndexIds: [], errors: [], normalizedChildItems: "", valid: true, warnings: [] });
  });

  it("keeps a 5k row profile list virtualized", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(5000));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    expect(screen.getAllByTestId("server-row").length).toBeLessThan(60);
    expect(screen.queryByText("Server 4999")).not.toBeInTheDocument();
  });

  it("keeps 500 rows responsive through 1 Hz live stat batches", () => {
    const profiles = makeProfiles(500);
    const startedAt = performance.now();
    let updated = profiles;

    for (let tick = 0; tick < 60; tick += 1) {
      const stats = Object.fromEntries(
        profiles.map((profile, index) => [
          profile.profile.IndexId,
          {
            ...profile.serverStat!,
            TodayDown: index * 2048 + tick,
            TodayUp: index * 1024 + tick,
            TotalDown: index * 8192 + tick,
            TotalUp: index * 4096 + tick,
          },
        ]),
      );

      updated = applyLiveUpdates(profiles, stats, {});
    }

    expect(performance.now() - startedAt).toBeLessThan(1000);
    expect(updated).toHaveLength(500);
    expect(updated[499].serverStat?.TodayDown).toBe(499 * 2048 + 59);
  });

  it("runs table operations through profile IPC wrappers", async () => {
    const profiles = makeProfiles(3);
    ipcMocks.listProfiles.mockResolvedValue(profiles);

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    await userEvent.click(screen.getByLabelText("Select Server 0"));
    await userEvent.click(screen.getByRole("button", { name: /Activate/ }));
    await userEvent.click(screen.getByRole("button", { name: /Copy/ }));
    await userEvent.click(screen.getByRole("button", { name: /Delete/ }));
    await userEvent.click(screen.getByRole("button", { name: "Remarks" }));

    expect(ipcMocks.setActiveProfile).toHaveBeenCalledWith("profile-0");
    expect(ipcMocks.copyProfiles).toHaveBeenCalledWith(["profile-0"]);
    expect(ipcMocks.deleteProfiles).toHaveBeenCalledWith(["profile-0"]);
    expect(ipcMocks.sortProfiles).toHaveBeenCalledWith(null, "remarks", true);
  });

  it("renders persisted traffic columns for profile rows", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(3));

    renderProfiles();

    expect(await screen.findByText("Today up")).toBeInTheDocument();
    expect(await screen.findByText("Server 1")).toBeInTheDocument();
    expect(screen.getAllByText("4.0 KB").length).toBeGreaterThan(0);
    expect(screen.getAllByText("8.0 KB").length).toBeGreaterThan(0);
  });

  it("runs import and subscription update actions through subscription IPC wrappers", async () => {
    ipcMocks.listProfiles.mockResolvedValue([]);

    renderProfiles();

    fireEvent.click(await screen.findByRole("button", { name: "Import" }));
    fireEvent.change(screen.getByLabelText("Import payload"), {
      target: { value: "vless://uuid@example.test:443#US" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Import payload" }));

    await waitFor(() =>
      expect(ipcMocks.importProfilesFromText).toHaveBeenCalledWith(
        "vless://uuid@example.test:443#US",
        null,
        false,
      ),
    );

    fireEvent.click(screen.getByRole("button", { name: "Update subs" }));
    expect(ipcMocks.updateSubscriptions).toHaveBeenCalledWith(null, false, null);
  });

  it("moves rows with drag and drop through the move IPC command", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(3));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    const rows = screen.getAllByTestId("server-row");
    const data = new Map<string, string>();
    const dataTransfer = {
      effectAllowed: "",
      getData: vi.fn((type: string) => data.get(type) ?? ""),
      setData: vi.fn((type: string, value: string) => data.set(type, value)),
    };

    fireEvent.dragStart(rows[0], { dataTransfer });
    fireEvent.dragOver(rows[1], { dataTransfer });
    fireEvent.drop(rows[1], { dataTransfer });

    await waitFor(() =>
      expect(ipcMocks.moveProfile).toHaveBeenCalledWith(null, "profile-0", MOVE_ACTIONS.Position, 1),
    );
  });

  it("submits every protocol through the zod-backed profile dialog path", async () => {
    ipcMocks.listProfiles.mockResolvedValue([]);

    renderProfiles();

    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await userEvent.click(screen.getByRole("combobox", { name: "Protocol" }));
    const protocolOptions = within(await screen.findByRole("listbox")).getAllByRole("option");
    expect(protocolOptions).toHaveLength(PROFILE_PROTOCOLS.length);
    PROFILE_PROTOCOLS.forEach((protocol) => {
      expect(screen.getByRole("option", { name: new RegExp(`^${escapeRegExp(protocol.label)}`) })).toBeInTheDocument();
    });

    await userEvent.click(screen.getByRole("option", { name: /^WireGuard/ }));
    expect(await screen.findByLabelText("Peer public key")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Remarks"), { target: { value: "WireGuard test" } });
    fireEvent.change(screen.getByLabelText("Address"), { target: { value: "wg.example.test" } });
    fireEvent.change(screen.getByLabelText("Private key"), { target: { value: "private-key" } });
    fireEvent.change(screen.getByLabelText("Peer public key"), { target: { value: "peer-key" } });
    fireEvent.click(screen.getByRole("button", { name: /Save/ }));

    await waitFor(() =>
      expect(ipcMocks.saveProfile).toHaveBeenCalledWith(
        expect.objectContaining({
          Address: "wg.example.test",
          ConfigType: CONFIG_TYPES.WireGuard,
          Password: "private-key",
          ProtocolExtra: expect.objectContaining({ WgPublicKey: "peer-key" }),
          Remarks: "WireGuard test",
        }),
      ),
    );
  });

  it("builds a policy group with child picker and generator preview", async () => {
    ipcMocks.listProfiles.mockResolvedValue([]);
    ipcMocks.listGroupChildCandidates.mockResolvedValue([
      {
        address: "a.example.test",
        configType: CONFIG_TYPES.VLESS,
        indexId: "leaf-a",
        isGroup: false,
        reason: null,
        remarks: "Leaf A",
        selectable: true,
        subid: "",
      },
      {
        address: "chain",
        configType: CONFIG_TYPES.ProxyChain,
        indexId: "chain-a",
        isGroup: true,
        reason: null,
        remarks: "Chain A",
        selectable: true,
        subid: "",
      },
    ]);
    ipcMocks.previewGroupProfile.mockResolvedValue({
      validation: {
        childIndexIds: ["leaf-a", "chain-a"],
        errors: [],
        normalizedChildItems: "leaf-a,chain-a",
        valid: true,
        warnings: [],
      },
      xrayRoutes: [
        {
          detour: null,
          dialerProxy: "chain-proxy-1-Chain A",
          downloadDialerProxy: null,
          kind: "vless",
          outbounds: [],
          tag: "proxy-1-Leaf A",
        },
      ],
      xrayBalancers: [{ fallbackTag: null, selectors: ["proxy"], strategy: "leastPing", tag: "proxy-balancer" }],
      xrayObservatorySelectors: ["proxy"],
      xrayBurstObservatorySelectors: [],
      singboxRoutes: [
        {
          detour: null,
          dialerProxy: null,
          downloadDialerProxy: null,
          kind: "selector",
          outbounds: ["proxy-auto", "proxy-1-Leaf A", "proxy-2-Chain A"],
          tag: "proxy",
        },
        {
          detour: null,
          dialerProxy: null,
          downloadDialerProxy: null,
          kind: "urltest",
          outbounds: ["proxy-1-Leaf A", "proxy-2-Chain A"],
          tag: "proxy-auto",
        },
      ],
    });

    renderProfiles();

    fireEvent.click(await screen.findByRole("button", { name: "Add" }));
    await selectComboboxOption("Protocol", "Policy Group");
    fireEvent.change(screen.getByLabelText("Remarks"), { target: { value: "Mixed policy" } });
    fireEvent.click(await screen.findByRole("button", { name: "Choose children" }));

    fireEvent.click(await screen.findByRole("checkbox", { name: /Leaf A/ }));
    fireEvent.click(screen.getByRole("checkbox", { name: /Chain A/ }));
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));

    fireEvent.click(screen.getByRole("button", { name: "Preview" }));
    expect(await screen.findByText("Xray dialerProxy")).toBeInTheDocument();
    expect(await screen.findByText("sing-box selector/urltest + detour")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Save/ }));

    await waitFor(() =>
      expect(ipcMocks.saveGroupProfile).toHaveBeenCalledWith(
        expect.objectContaining({
          ConfigType: CONFIG_TYPES.PolicyGroup,
          ProtocolExtra: expect.objectContaining({
            ChildItems: "leaf-a,chain-a",
            GroupType: "PolicyGroup",
          }),
          Remarks: "Mixed policy",
        }),
      ),
    );
  });
});

function makeProfiles(count: number) {
  return Array.from({ length: count }, (_, index) => makeProfile(index));
}

function makeProfile(index: number, overrides: ProfileItem_Deserialize = {}): ProfileListItem_Serialize {
  const indexId = overrides.IndexId ?? `profile-${index}`;

  return {
    isActive: index === 0,
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
      Delay: index % 2 === 0 ? 40 + index : 0,
      IndexId: indexId,
      IpInfo: index % 2 === 0 ? "US" : null,
      Message: null,
      Sort: index * 10,
      Speed: index % 2 === 0 ? 2048 : null,
    },
    serverStat: {
      DateNow: 1,
      IndexId: indexId,
      TodayDown: index * 2048,
      TodayUp: index * 1024,
      TotalDown: index * 8192,
      TotalUp: index * 4096,
    },
  };
}

function makeSubscription() {
  return {
    AutoUpdateInterval: 0,
    Enabled: true,
    Filter: null,
    Id: "sub-1",
    Memo: null,
    MoreUrl: "",
    NextProfile: null,
    PreSocksPort: null,
    PrevProfile: null,
    Remarks: "Fixture",
    Sort: 1,
    UpdateTime: 0,
    Url: "https://example.test/sub",
    UserAgent: "",
  };
}
