import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, vi } from "vitest";

import type { ProfileItem_Deserialize, ProfileListItem_Serialize } from "@/ipc/bindings";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { DEFAULT_PROFILE_COLUMN_VISIBILITY, useProfileColumnsStore } from "@/stores/profile-columns-store";

import { CONFIG_TYPES, MOVE_ACTIONS, PROFILE_PROTOCOLS, SPEED_ACTIONS } from "./profile-constants";
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
  cancelSpeedtest: vi.fn(),
  runSpeedtest: vi.fn(),
  saveGroupProfile: vi.fn(),
  saveProfile: vi.fn(),
  saveSubscription: vi.fn(),
  setActiveProfile: vi.fn(),
  sortProfiles: vi.fn(),
  updateSubscriptions: vi.fn(),
  validateGroupProfile: vi.fn(),
}));

vi.mock("@/ipc", async () => {
  const runtimeStore = await vi.importActual<typeof import("@/ipc/runtime-event-store")>(
    "@/ipc/runtime-event-store",
  );

  return {
    ...ipcMocks,
    useRuntimeEventStore: runtimeStore.useRuntimeEventStore,
  };
});

const queryClients = new Set<QueryClient>();
const originalClipboardDescriptor = Object.getOwnPropertyDescriptor(navigator, "clipboard");

function renderProfiles() {
  const queryClient = new QueryClient({
    defaultOptions: {
      mutations: { retry: false },
      queries: { gcTime: 0, retry: false },
    },
  });

  queryClients.add(queryClient);

  return render(
    <QueryClientProvider client={queryClient}>
      <ProfilesScreen />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  queryClients.forEach((queryClient) => queryClient.clear());
  queryClients.clear();
  restoreClipboard();
});

function mockClipboardReadText(text: string) {
  const readText = vi.fn<() => Promise<string>>().mockResolvedValue(text);

  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { readText },
  });

  return readText;
}

function mockClipboardUnavailable() {
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: undefined,
  });
}

function restoreClipboard() {
  if (originalClipboardDescriptor) {
    Object.defineProperty(navigator, "clipboard", originalClipboardDescriptor);
    return;
  }

  Reflect.deleteProperty(navigator, "clipboard");
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
    // Column visibility persists to localStorage, so reset it between tests to
    // keep the default-column expectations independent of prior toggles.
    useProfileColumnsStore.setState({ columnVisibility: { ...DEFAULT_PROFILE_COLUMN_VISIBILITY } });
    useRuntimeEventStore.setState({
      serverStatsByProfileId: {},
      speedtestResultsByProfileId: {},
      speedtestRunning: false,
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
      singboxRoutes: [],
    });
    ipcMocks.cancelSpeedtest.mockResolvedValue({ running: false });
    ipcMocks.runSpeedtest.mockResolvedValue({
      action: SPEED_ACTIONS.Speedtest,
      cancelled: false,
      completedCount: 0,
      results: [],
      selectedCount: 0,
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

  it("shows speedtest status messages even when a previous speed value exists", async () => {
    const profile = makeProfile(0);
    profile.profileEx = {
      ...profile.profileEx,
      Delay: -1,
      IpInfo: "Skipped",
      Message: "request timed out",
      Speed: 2048,
    };
    ipcMocks.listProfiles.mockResolvedValue([profile]);

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    expect(screen.queryByRole("columnheader", { name: "Speed" })).not.toBeInTheDocument();

    // Speed is now a probe inside the speedtest split-button menu.
    await userEvent.click(screen.getByRole("menuitem", { name: "More speed tests" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Speed" }));

    expect(await screen.findByRole("columnheader", { name: "Speed" })).toBeInTheDocument();
    expect(await screen.findByText("request timed out")).toBeInTheDocument();
    expect(screen.queryByText("2.0 KB/s")).not.toBeInTheDocument();
  });

  it("runs table operations through profile IPC wrappers", async () => {
    const profiles = makeProfiles(3);
    ipcMocks.listProfiles.mockResolvedValue(profiles);

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    await userEvent.click(screen.getByLabelText("Select Server 0"));
    await userEvent.click(screen.getByRole("button", { name: /Activate/ }));
    await userEvent.click(screen.getByRole("button", { name: /Copy/ }));
    // Delete now routes through a confirmation dialog before the IPC call fires.
    await userEvent.click(screen.getByRole("button", { name: /Delete/ }));
    const confirm = await screen.findByRole("alertdialog");
    fireEvent.click(within(confirm).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    await userEvent.click(screen.getByRole("button", { name: "Remarks" }));

    expect(ipcMocks.setActiveProfile).toHaveBeenCalledWith("profile-0");
    expect(ipcMocks.copyProfiles).toHaveBeenCalledWith(["profile-0"]);
    expect(ipcMocks.deleteProfiles).toHaveBeenCalledWith(["profile-0"]);
    expect(ipcMocks.sortProfiles).toHaveBeenCalledWith(null, "remarks", true);
  });

  it("re-enables speedtest buttons when the speedtest IPC rejects", async () => {
    let rejectSpeedtest: (reason?: unknown) => void = () => {};
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(1));
    ipcMocks.runSpeedtest.mockReturnValue(
      new Promise((_resolve, reject) => {
        rejectSpeedtest = reject;
      }),
    );

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    await userEvent.click(screen.getByLabelText("Select Server 0"));

    // The split button's default Fast control stays a button, so it can hold the
    // disabled/enabled state across the running speedtest the way the per-action
    // buttons used to.
    const speedButton = screen.getByRole("button", { name: "Fast" });
    await userEvent.click(speedButton);

    await waitFor(() => expect(speedButton).toBeDisabled());
    expect(ipcMocks.runSpeedtest).toHaveBeenCalledWith(SPEED_ACTIONS.FastRealping, ["profile-0"]);

    rejectSpeedtest(new Error("boom"));

    await waitFor(() => expect(speedButton).toBeEnabled());
  });

  it("runs realping for all profiles when no rows are selected", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(2));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    // Real ping moved into the speedtest split-button menu.
    await userEvent.click(screen.getByRole("menuitem", { name: "More speed tests" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Real" }));

    expect(ipcMocks.runSpeedtest).toHaveBeenCalledWith(SPEED_ACTIONS.Realping, []);
  });

  it("reflects an already running speedtest from the runtime store", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(1));
    useRuntimeEventStore.setState({ speedtestRunning: true });

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    await waitFor(() => expect(screen.getByRole("button", { name: "Fast" })).toBeDisabled());

    // Stop is reachable through the split-button menu and stays enabled while a
    // run is in flight, even as the probe items are disabled.
    await userEvent.click(screen.getByRole("menuitem", { name: "More speed tests" }));
    const stopItem = await screen.findByRole("menuitem", { name: "Stop" });
    expect(stopItem).not.toHaveAttribute("data-disabled");
  });

  it("confirms before deleting and cancels without calling the delete IPC", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(3));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    await userEvent.click(screen.getByLabelText("Select Server 0"));
    await userEvent.click(screen.getByRole("button", { name: /Delete/ }));

    const confirm = await screen.findByRole("alertdialog");
    fireEvent.click(within(confirm).getByRole("button", { name: "Cancel" }));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(ipcMocks.deleteProfiles).not.toHaveBeenCalled();
  });

  it("shows a localized empty state when no profiles exist", async () => {
    ipcMocks.listProfiles.mockResolvedValue([]);

    renderProfiles();

    expect(await screen.findByText("No profiles")).toBeInTheDocument();
    expect(
      screen.getByText("Add a profile or import one from a subscription to get started."),
    ).toBeInTheDocument();
    expect(screen.queryByTestId("server-row")).not.toBeInTheDocument();
  });

  it("ships high-signal columns and collapses niche ones by default", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(3));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    for (const label of ["Protocol", "Remarks", "Address", "Delay", "Group"]) {
      expect(screen.getByRole("columnheader", { name: label })).toBeInTheDocument();
    }
    expect(screen.queryByRole("columnheader", { name: "IP info" })).not.toBeInTheDocument();
    expect(screen.queryByRole("columnheader", { name: "Security" })).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("menuitem", { name: "Columns" }));
    await userEvent.click(await screen.findByRole("menuitemcheckbox", { name: "IP info" }));
    expect(await screen.findByRole("columnheader", { name: "IP info" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("menuitemcheckbox", { name: "IP info" }));
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("columnheader", { name: "IP info" })).not.toBeInTheDocument();
  });

  it("reveals niche traffic columns through the column menu", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(3));

    renderProfiles();

    expect(await screen.findByText("Server 1")).toBeInTheDocument();
    // Traffic columns are collapsed by default to cut horizontal scroll.
    expect(screen.queryByRole("columnheader", { name: "Total up" })).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("menuitem", { name: "Columns" }));
    await userEvent.click(await screen.findByRole("menuitemcheckbox", { name: "Total up" }));
    await userEvent.keyboard("{Escape}");

    expect(await screen.findByRole("columnheader", { name: "Total up" })).toBeInTheDocument();
    expect(screen.getAllByText("4.0 KB").length).toBeGreaterThan(0);
    expect(screen.getAllByText("8.0 KB").length).toBeGreaterThan(0);
  });

  it("restores default columns from the column menu reset action", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(3));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("menuitem", { name: "Columns" }));
    await userEvent.click(await screen.findByRole("menuitemcheckbox", { name: "IP info" }));
    expect(await screen.findByRole("columnheader", { name: "IP info" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("menuitem", { name: "Reset to defaults" }));
    expect(screen.queryByRole("columnheader", { name: "IP info" })).not.toBeInTheDocument();
  });

  it("runs import and subscription update actions through subscription IPC wrappers", async () => {
    ipcMocks.listProfiles.mockResolvedValue([]);

    renderProfiles();

    // Import and subscription actions now live in the toolbar overflow menu.
    // Trigger the subscription update first: a successful import keeps the dialog
    // open to show its result summary, and the open modal makes the toolbar
    // aria-hidden, so "Update subs" must be exercised before importing.
    await userEvent.click(await screen.findByRole("menuitem", { name: "More actions" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Update subs" }));
    expect(ipcMocks.updateSubscriptions).toHaveBeenCalledWith(null, false, null);

    await userEvent.click(screen.getByRole("menuitem", { name: "More actions" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Import" }));
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
  });

  it("imports profiles directly from clipboard text", async () => {
    const clipboardText = "vless://uuid@example.test:443#US";
    const readText = mockClipboardReadText(`\n${clipboardText}\n`);
    ipcMocks.listProfiles.mockResolvedValue([]);

    renderProfiles();

    await userEvent.click(await screen.findByRole("menuitem", { name: "More actions" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Import from clipboard" }));

    await waitFor(() => expect(readText).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(ipcMocks.importProfilesFromText).toHaveBeenCalledWith(clipboardText, null, false),
    );
    expect(await screen.findByText("Imported 1 profile(s) from clipboard.")).toBeInTheDocument();
  });

  it("does not import when clipboard text is empty", async () => {
    const readText = mockClipboardReadText(" \n ");
    ipcMocks.listProfiles.mockResolvedValue([]);

    renderProfiles();

    await userEvent.click(await screen.findByRole("menuitem", { name: "More actions" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Import from clipboard" }));

    await waitFor(() => expect(readText).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("Clipboard is empty.")).toBeInTheDocument();
    expect(ipcMocks.importProfilesFromText).not.toHaveBeenCalled();
  });

  it("does not import when clipboard text read is unavailable", async () => {
    mockClipboardUnavailable();
    ipcMocks.listProfiles.mockResolvedValue([]);

    renderProfiles();

    await userEvent.click(await screen.findByRole("menuitem", { name: "More actions" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Import from clipboard" }));

    expect(await screen.findByText("Clipboard text read is unavailable in this WebView.")).toBeInTheDocument();
    expect(ipcMocks.importProfilesFromText).not.toHaveBeenCalled();
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
    const user = userEvent.setup();

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

    await user.click(await screen.findByRole("button", { name: "Add" }));
    await selectComboboxOption("Protocol", "Policy Group");
    fireEvent.change(screen.getByLabelText("Remarks"), { target: { value: "Mixed policy" } });
    await user.click(await screen.findByRole("button", { name: "Choose children" }));

    await user.click(await screen.findByRole("checkbox", { name: /Leaf A/ }));
    await user.click(screen.getByRole("checkbox", { name: /Chain A/ }));
    await user.click(screen.getByRole("button", { name: "Apply" }));

    expect(await screen.findByText("Leaf A")).toBeInTheDocument();
    expect(await screen.findByText("Chain A")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Preview" }));
    expect(await screen.findByText("sing-box selector/urltest + detour")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Save/ }));

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
  }, 10_000);
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
