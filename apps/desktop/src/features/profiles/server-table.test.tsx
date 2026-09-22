import {
  act,
  cleanup,
  fireEvent,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { QueryClient } from "@tanstack/react-query";
import { createTestQueryClient, renderHookWithQuery, renderWithQuery } from "@/test/render";
import { afterEach, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { useNodeListStore } from "@voya/client/node-list-store";

import { IpcCommandError } from "@voya/client/errors";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import type {
  ImportProfilesResult,
  Profile,
  ProfileDetails,
  ProfileSummaryEntry,
  RuntimeStatusResponse,
  SpeedtestResult,
  TunStatus,
} from "@/ipc/bindings";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useToastStore } from "@voya/client/toast-store";
import { makeProfileDetailsFixture, toProfileSummaryEntry } from "@voya/features/test/profile-fixture";

import { MOVE_ACTIONS } from "@voya/features/profiles/profile-constants";
import { ProfilesScreen } from "./server-table";
import { applySpeedtestResults, overlaySpeedtestResult, useNodeListData } from "@voya/features/profiles/use-node-list-data";
import { useNodeGroups } from "./use-node-groups";

const ipcMocks = vi.hoisted(() => ({
  connectActiveProfile: vi.fn(),
  restartCore: vi.fn(),
  runtimeStatus: vi.fn(),
  systemProxyStatus: vi.fn(),
  tunStatus: vi.fn(),
  tunRequestElevation: vi.fn(),
  deleteSubscriptions: vi.fn(),
  deleteProfiles: vi.fn(),
  exportProfileShareLinks: vi.fn(),
  generateQrCode: vi.fn(),
  getProfile: vi.fn(),
  importProfilesFromText: vi.fn(),
  readClipboardText: vi.fn(),
  listProfileSummaries: vi.fn(),
  listPolicyGroups: vi.fn(),
  policyGroupRuntime: vi.fn(),
  deletePolicyGroups: vi.fn(),
  savePolicyGroup: vi.fn(),
  selectPolicyGroupMember: vi.fn(),
  setActivePolicyGroup: vi.fn(),
  testPolicyGroupDelay: vi.fn(),
  listSubscriptionMetadata: vi.fn(() => Promise.resolve([])),
  listSubscriptions: vi.fn(),
  moveProfile: vi.fn(),
  cancelSpeedtest: vi.fn(),
  runSpeedtest: vi.fn(),
  scanScreenQr: vi.fn(),
  saveProfile: vi.fn(),
  saveSubscription: vi.fn(),
  setActiveProfile: vi.fn(),
  updateSubscriptions: vi.fn(),
}));

// The real error class and kind check, so a missing core opens its dialog.
vi.mock("@/ipc/commands", async () => {
  return { ...ipcMocks };
});

// `listProfileSummaries` answers with the rows plus the number of stored profiles this
// build could not decode. Tests that only care about the rows go through these
// helpers, so the shape lives in one place instead of every mock.
function listing(entries: ProfileSummaryEntry[], undecodableProfiles = 0) {
  return { entries, undecodableProfiles };
}

function mockProfileList(entries: ProfileSummaryEntry[], undecodableProfiles = 0) {
  ipcMocks.listProfileSummaries.mockResolvedValue(
    listing(entries, undecodableProfiles),
  );
}

function mockProfileListOnce(
  entries: ProfileSummaryEntry[],
  undecodableProfiles = 0,
) {
  ipcMocks.listProfileSummaries.mockResolvedValueOnce(
    listing(entries, undecodableProfiles),
  );
}

const queryClients = new Set<QueryClient>();

function renderProfiles() {
  const queryClient = createTestQueryClient({ gcTime: 0 });
  queryClients.add(queryClient);
  return renderWithQuery(<ProfilesScreen />, { queryClient });
}

afterEach(() => {
  queryClients.forEach((queryClient) => queryClient.clear());
  queryClients.clear();
  ipcMocks.readClipboardText.mockReset();
});

function mockClipboardReadText(text: string) {
  ipcMocks.readClipboardText.mockResolvedValue(text);

  return ipcMocks.readClipboardText;
}

function mockClipboardUnavailable() {
  ipcMocks.readClipboardText.mockRejectedValue(
    new Error("the clipboard is not supported in this session"),
  );
}

async function selectComboboxOption(label: string, optionLabel: string) {
  const user = userEvent.setup();

  await user.click(screen.getByRole("combobox", { name: label }));
  const listbox = await screen.findByRole("listbox");
  await user.click(
    within(listbox).getByRole("option", {
      name: new RegExp(`^${escapeRegExp(optionLabel)}`),
    }),
  );
}

// Locale switches never touch localStorage, and the default locale is restored
// even when the body throws, so one test cannot leak a language into the next.
async function withLocale(locale: "en" | "zh-Hans", body: () => Promise<void>) {
  await changeLocale(locale, { persist: false });
  try {
    await body();
  } finally {
    // Unmount first: restoring the language while the tree is still mounted
    // would re-render it outside act().
    cleanup();
    await changeLocale("en", { persist: false });
  }
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

async function openRowContextMenu(rowIndex = 0) {
  const row = screen.getAllByTestId("server-row")[rowIndex];
  if (!row) {
    throw new Error(`Profile row ${rowIndex} is not rendered`);
  }
  fireEvent.contextMenu(row, { clientX: 120, clientY: 80 });
  return screen.findByRole("menu");
}

async function openContextSubmenu(menu: HTMLElement, name: string) {
  const trigger = within(menu).getByRole("menuitem", { name });
  fireEvent.focus(trigger);
  fireEvent.keyDown(trigger, { key: "ArrowRight" });
  await waitFor(() => expect(trigger).toHaveAttribute("aria-expanded", "true"));
  const menus = await screen.findAllByRole("menu");
  return menus.at(-1)!;
}

describe("ProfilesScreen", () => {
  beforeEach(() => {
    useRuntimeActionStore.setState({
      pendingAction: null,
      modePending: false,
      switchingId: null,
    });
    Object.values(ipcMocks).forEach((mock) => {
      if ("mockReset" in mock) {
        mock.mockReset();
      }
    });
    ipcMocks.listPolicyGroups.mockResolvedValue({ entries: [] });
    // The list carries summaries; the editor reads the node it opens in full.
    detailsById.clear();
    ipcMocks.getProfile.mockImplementation((id: string) => {
      const details = detailsById.get(id);
      return details ? Promise.resolve(details) : Promise.reject(new Error(`node ${id} not found`));
    });
    window.localStorage.removeItem("voyavpn.profileColumns");
    useToastStore.setState({ toasts: [] });
    useRuntimeActionStore.setState({ missingCore: null });
    useRuntimeEventStore.setState({
      coreState: null,
      serverStatsByProfileId: {},
      speedtestResultsByProfileId: {},
      speedtestRunning: false,
    });
    ipcMocks.connectActiveProfile.mockResolvedValue({
      state: "connected",
      activeProfileId: "profile-0",
    });
    ipcMocks.restartCore.mockResolvedValue({
      state: "connected",
      activeProfileId: "profile-0",
    });
    ipcMocks.runtimeStatus.mockImplementation(
      async () => useRuntimeEventStore.getState().coreState,
    );
    ipcMocks.systemProxyStatus.mockResolvedValue(null);
    ipcMocks.tunStatus.mockResolvedValue(null);
    ipcMocks.deleteSubscriptions.mockResolvedValue(1);
    ipcMocks.deleteProfiles.mockResolvedValue(1);
    ipcMocks.exportProfileShareLinks.mockImplementation(
      async (indexIds: string[]) => ({
        count: indexIds.length,
        text: indexIds
          .map((indexId) => `vless://${indexId}@example.test:443`)
          .join("\n"),
      }),
    );
    ipcMocks.generateQrCode.mockResolvedValue({
      mimeType: "image/svg+xml",
      svg: "<svg />",
    });
    ipcMocks.importProfilesFromText.mockResolvedValue(
      makeImportResult({ imported: 1, importedProfileIds: ["profile-new"] }),
    );
    ipcMocks.listSubscriptions.mockResolvedValue([]);
    ipcMocks.moveProfile.mockResolvedValue([]);
    ipcMocks.cancelSpeedtest.mockResolvedValue({ running: false });
    ipcMocks.runSpeedtest.mockResolvedValue({
      cancelled: false,
      completedCount: 0,
      results: [],
      selectedCount: 0,
    });
    ipcMocks.scanScreenQr.mockResolvedValue({
      message: null,
      source: "screen",
      status: "unavailable",
      texts: [],
      failureReason: null,
    });
    ipcMocks.saveProfile.mockImplementation(async (profile: Profile) =>
      makeProfile(99, profile),
    );
    ipcMocks.saveSubscription.mockResolvedValue(makeSubscription());
    ipcMocks.setActiveProfile.mockImplementation(async (profileId: string) =>
      makeProfile(0, { id: profileId }),
    );
    ipcMocks.updateSubscriptions.mockResolvedValue({
      imported: 0,
      messages: [],
      removedExisting: 0,
      skipped: 0,
      updated: 0,
    });
  });

  it("keeps a 5k row profile list virtualized", async () => {
    mockProfileList(makeProfiles(5000));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    expect(screen.getAllByRole("listitem")[0]).toHaveAttribute(
      "aria-setsize",
      "5001",
    );
    expect(screen.getAllByTestId("server-row").length).toBeLessThan(60);
    expect(screen.queryByText("Server 4999")).not.toBeInTheDocument();
  });

  it("merges repeated speedtest batches for 500 rows without rebuilding their traffic", () => {
    const profiles = makeProfiles(500);
    const startedAt = performance.now();
    let updated = profiles;

    for (let tick = 0; tick < 60; tick += 1) {
      const results: Record<string, SpeedtestResult> = Object.fromEntries(
        profiles.map((profile, index) => [profile.profile.id, {
          indexId: profile.profile.id,
          delay: index + tick,
          outcome: "completed",
          detail: null,
          ipInfo: null,
          countryCode: null,
        }]),
      );
      updated = applySpeedtestResults(profiles, results);
    }

    expect(performance.now() - startedAt).toBeLessThan(1000);
    expect(updated).toHaveLength(500);
    expect(updated[499].metrics.delayMs).toBe(499 + 59);
    expect(updated[499].profile).toBe(profiles[499].profile);
  });

  it("preserves unmatched rows, missing measurements and authoritative country data", () => {
    const profiles = makeProfiles(2);
    const original = profiles[0]!;
    original.metrics = { ...original.metrics, delayMs: 42, ipInfo: "saved IP", countryCode: "JP" };
    const result: SpeedtestResult = {
      indexId: original.profile.id, delay: null, ipInfo: null,
      countryCode: "US", outcome: "timedOut", detail: null,
    };
    expect(applySpeedtestResults(profiles, {})).toBe(profiles);
    expect(applySpeedtestResults(profiles, { missing: result })).toBe(profiles);
    const updated = applySpeedtestResults(profiles, { [original.profile.id]: result });
    expect(updated[1]).toBe(profiles[1]);
    expect(updated[0].metrics).toMatchObject({ delayMs: 42, ipInfo: "saved IP", countryCode: "JP", outcome: "timedOut" });
    expect(original.metrics.outcome).not.toBe("timedOut");
    const measured = applySpeedtestResults(profiles, {
      [original.profile.id]: { ...result, delay: 0, ipInfo: "new IP", outcome: "completed" },
    });
    expect(measured[0].metrics).toMatchObject({ delayMs: 0, ipInfo: "new IP", outcome: "completed" });
  });

  it("overlays a live result once and keeps the row's identity across frames", () => {
    const [item] = makeProfiles(1);
    const result: SpeedtestResult = {
      indexId: item!.profile.id, delay: 55, ipInfo: null,
      countryCode: null, outcome: "completed", detail: null,
    };

    const overlaid = overlaySpeedtestResult(item!, result);
    expect(overlaid).not.toBe(item);
    expect(overlaid.metrics).toMatchObject({ delayMs: 55, outcome: "completed" });
    // The same pair on a later frame is the same object, and a row that
    // already reads as the result is left alone.
    expect(overlaySpeedtestResult(item!, result)).toBe(overlaid);
    expect(overlaySpeedtestResult(overlaid, result)).toBe(overlaid);
    expect(overlaySpeedtestResult(item!, undefined)).toBe(item);
  });

  it("lays rows out from the listing until the view sorts by latency", async () => {
    useNodeListStore.setState({ hideUnreachable: false, sortByLatency: false });
    mockProfileList(makeProfiles(3));
    const { result } = renderHookWithQuery(() => {
      const { t } = useI18n();
      return useNodeListData(useNodeGroups(), t);
    });
    await waitFor(() => expect(result.current.rows).toHaveLength(4));
    const before = result.current.rows;

    act(() =>
      useRuntimeEventStore.setState({
        speedtestResultsByProfileId: {
          "profile-2": {
            indexId: "profile-2", delay: 77, ipInfo: null,
            countryCode: null, outcome: "completed", detail: null,
          },
        },
      }),
    );

    // A result frame reaches the node, not the layout: the rendered row
    // overlays it.
    expect(result.current.rows).toBe(before);
    expect(result.current.profiles[2]!.metrics.delayMs).toBe(77);
    const row = before.find((entry) => entry.kind === "profile" && entry.item.profile.id === "profile-2");
    expect(row?.kind).toBe("profile");
    if (row?.kind === "profile") {
      const live = result.current.speedtestResultsByProfileId["profile-2"];
      expect(overlaySpeedtestResult(row.item, live).metrics.delayMs).toBe(77);
    }

    act(() => {
      useNodeListStore.setState({ sortByLatency: true });
    });
    const order = result.current.rows.flatMap((entry) => (entry.kind === "profile" ? [entry.item.profile.id] : []));
    expect(order[0]).toBe("profile-2");

    useNodeListStore.setState({ sortByLatency: false });
    useRuntimeEventStore.setState({ speedtestResultsByProfileId: {} });
  });

  it.each([
    ["waiting", "Waiting"],
    ["testing", "Testing"],
    ["timedOut", "Request timed out"],
    ["cancelled", "Cancelled"],
  ] as const)(
    "shows %s instead of a previous latency",
    async (outcome, label) => {
      const profile = makeProfile(0);
      profile.metrics = { ...profile.metrics, delayMs: 42, outcome };
      mockProfileList([profile]);
      renderProfiles();
      expect(await screen.findByText("Server 0")).toBeInTheDocument();
      expect(screen.getByText(label)).toBeInTheDocument();
      expect(screen.queryByText("42 ms")).not.toBeInTheDocument();
      expect(
        screen.queryByRole("columnheader", { name: "Speed" }),
      ).not.toBeInTheDocument();
    },
  );

  it("keeps row actions without multi-select, copy, sorting, or dedupe", async () => {
    const profiles = makeProfiles(3);
    mockProfileList(profiles);

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    const rows = screen.getAllByTestId("server-row");
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Copy" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("menuitem", { name: "Sort" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Sort" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Dedupe" }),
    ).not.toBeInTheDocument();

    // Rows carry no hidden selection: clicking one changes nothing it shows.
    await userEvent.click(rows[0]!);
    expect(rows[0]).not.toHaveAttribute("data-selected");
    expect(
      screen.queryByRole("button", { name: /Activate/ }),
    ).not.toBeInTheDocument();

    const menu = await openRowContextMenu(0);
    await userEvent.click(
      within(menu).getByRole("menuitem", { name: "Delete" }),
    );
    const confirm = await screen.findByRole("alertdialog");
    fireEvent.click(within(confirm).getByRole("button", { name: "Delete" }));
    await waitFor(() =>
      expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(),
    );

    expect(ipcMocks.setActiveProfile).not.toHaveBeenCalled();
    expect(ipcMocks.deleteProfiles).toHaveBeenCalledWith(["profile-0"]);
  });

  it("opens the targeted profile editor from the row context menu", async () => {
    mockProfileList(makeProfiles(2));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    const menu = await openRowContextMenu(1);
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Edit" }));

    const dialog = await screen.findByRole("dialog", { name: "Edit node" });
    expect(within(dialog).getByLabelText("Remarks")).toHaveValue("Server 1");
  });

  it("reports a failed node read instead of opening an empty editor", async () => {
    mockProfileList(makeProfiles(2));
    ipcMocks.getProfile.mockRejectedValue(new Error("node profile-1 not found"));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    const menu = await openRowContextMenu(1);
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Edit" }));

    expect(await screen.findByText("node profile-1 not found")).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Edit node" })).not.toBeInTheDocument();
  });

  it("opens the node asked for last when two reads overlap", async () => {
    mockProfileList(makeProfiles(2));
    // Node 0's read answers after node 1's, as a slower first request would.
    const pending = new Map<string, (details: ProfileDetails) => void>();
    ipcMocks.getProfile.mockImplementation(
      (id: string) => new Promise<ProfileDetails>((resolve) => pending.set(id, resolve)),
    );

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    const first = await openRowContextMenu(0);
    await userEvent.click(within(first).getByRole("menuitem", { name: "Edit" }));
    const second = await openRowContextMenu(1);
    await userEvent.click(within(second).getByRole("menuitem", { name: "Edit" }));

    await waitFor(() => expect(pending.size).toBe(2));
    act(() => {
      pending.get("profile-1")?.(detailsById.get("profile-1") as ProfileDetails);
      pending.get("profile-0")?.(detailsById.get("profile-0") as ProfileDetails);
    });

    const dialog = await screen.findByRole("dialog", { name: "Edit node" });
    expect(within(dialog).getByLabelText("Remarks")).toHaveValue("Server 1");
  });

  it("re-enables speedtest buttons when the speedtest IPC rejects", async () => {
    let rejectSpeedtest: (reason?: unknown) => void = () => {};
    mockProfileList(makeProfiles(1));
    ipcMocks.runSpeedtest.mockReturnValue(
      new Promise((_resolve, reject) => {
        rejectSpeedtest = reject;
      }),
    );

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    const speedButton = screen.getByRole("button", { name: "Test group" });
    await userEvent.click(speedButton);

    await waitFor(() => expect(speedButton).toHaveAccessibleName(/^Stop/));
    expect(ipcMocks.runSpeedtest).toHaveBeenCalledWith({
      target: { scope: "profiles", profileIds: ["profile-0"] },
    });

    rejectSpeedtest(new Error("boom"));

    await waitFor(() => expect(speedButton).toHaveAccessibleName("Test group"));
    expect(speedButton).toBeEnabled();
  });

  it("runs a row speedtest only for the context-menu target", async () => {
    mockProfileList(makeProfiles(2));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    const menu = await openRowContextMenu();
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Ping" }));

    expect(ipcMocks.runSpeedtest).toHaveBeenCalledWith({
      target: { scope: "profiles", profileIds: ["profile-0"] },
    });
  });

  it("always displays latency despite old column preferences", async () => {
    window.localStorage.setItem(
      "voyavpn.profileColumns",
      JSON.stringify({
        state: { columnVisibility: { delay: false, remarks: false } },
      }),
    );
    const profile = makeProfile(0);
    profile.metrics.delayMs = 42;
    mockProfileList([profile]);
    renderProfiles();
    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    expect(screen.getByText("42 ms")).toBeInTheDocument();
    expect(
      screen.queryByRole("menuitem", { name: "Columns" }),
    ).not.toBeInTheDocument();
  });

  it("offers Stop for an existing run and disables the row action", async () => {
    mockProfileList(makeProfiles(1));
    useRuntimeEventStore.setState({ speedtestRunning: true });
    renderProfiles();
    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /^Stop/ })[0]!).toBeEnabled();
    const menu = await openRowContextMenu();
    expect(
      within(menu).getByRole("menuitem", { name: "Ping" }),
    ).toHaveAttribute("data-disabled");
    expect(ipcMocks.runSpeedtest).not.toHaveBeenCalled();
  });

  it("waits for the run to finish after cancellation is requested", async () => {
    let finishRun!: (
      value: Awaited<ReturnType<typeof ipcMocks.runSpeedtest>>,
    ) => void;
    ipcMocks.runSpeedtest.mockReturnValue(
      new Promise((resolve) => {
        finishRun = resolve;
      }),
    );
    ipcMocks.cancelSpeedtest.mockResolvedValue({ running: true });
    mockProfileList(makeProfiles(1));
    renderProfiles();
    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Test group" }));
    await userEvent.click(
      (await screen.findAllByRole("button", { name: /^Stop/ }))[0]!,
    );
    expect(ipcMocks.cancelSpeedtest).toHaveBeenCalledOnce();
    expect(
      screen.getAllByRole("button", { name: /^Stop/ })[0]!,
    ).toBeInTheDocument();
    expect(ipcMocks.runSpeedtest).toHaveBeenCalledOnce();
    finishRun({
      cancelled: true,
      completedCount: 0,
      selectedCount: 1,
      results: [],
    });
    expect(
      await screen.findByRole("button", { name: "Test group" }),
    ).toBeEnabled();
  });

  it("confirms before deleting and cancels without calling the delete IPC", async () => {
    mockProfileList(makeProfiles(3));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    const menu = await openRowContextMenu();
    await userEvent.click(
      within(menu).getByRole("menuitem", { name: "Delete" }),
    );

    const confirm = await screen.findByRole("alertdialog");
    fireEvent.click(within(confirm).getByRole("button", { name: "Cancel" }));

    await waitFor(() =>
      expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(),
    );
    expect(ipcMocks.deleteProfiles).not.toHaveBeenCalled();
  });

  it("shows a localized empty state when no profiles exist", async () => {
    mockProfileList([]);

    renderProfiles();

    expect(await screen.findByText("No nodes")).toBeInTheDocument();
    expect(
      screen.getByText(
        "Add a node or import one from a subscription to get started.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByTestId("server-row")).not.toBeInTheDocument();
  });

  it("shows a compact card with subscription name, details and an accessible selection button", async () => {
    const profile = makeProfile(0, {
      subscriptionId: "sub-1",
      remarks: "🇯🇵 Tokyo",
    });
    profile.metrics.delayMs = 42;
    profile.metrics.ipInfo = "Tokyo IP";
    mockProfileList([profile]);
    ipcMocks.listSubscriptions.mockResolvedValue([
      { ...makeSubscription(), id: "sub-1", remarks: "Travel" },
    ]);
    renderProfiles();
    expect(await screen.findByText("Tokyo")).toBeInTheDocument();
    expect(await screen.findByText("Travel")).toBeInTheDocument();
    expect(screen.getByText("42 ms")).toBeInTheDocument();
    expect(screen.queryByText("Tokyo IP")).not.toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    // The name opens the node's details from the keyboard, without using the node.
    const name = screen.getByRole("button", { name: "Details for 🇯🇵 Tokyo" });
    name.focus();
    await userEvent.keyboard(" ");
    expect(await screen.findByRole("dialog", { name: "Node details" })).toBeInTheDocument();
    expect(ipcMocks.setActiveProfile).not.toHaveBeenCalled();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(name).toHaveFocus());
    // The name is the only way into details; rows carry no separate "Details" link.
    expect(screen.queryByRole("button", { name: "Details" })).not.toBeInTheDocument();
    await userEvent.click(name);
    const dialog = screen.getByRole("dialog", { name: "Node details" });
    expect(dialog).toHaveTextContent("Tokyo IP");
    expect(dialog).toHaveTextContent("443");
    expect(dialog).toHaveTextContent("Travel");
    await userEvent.keyboard("{Escape}");
    expect(name).toHaveFocus();
  });

  it("reads transport, security and stored traffic in full when details open", async () => {
    mockProfileList([
      makeProfile(3, {
        tls: {
          alpn: [],
          certificatePem: null,
          echConfig: [],
          mode: "tls",
          realityPublicKey: null,
          realityShortId: null,
          serverName: null,
        },
        transport: { host: null, kind: "websocket", path: "/ws" },
      }),
    ]);
    renderProfiles();
    await screen.findByText("Server 3");
    // The list alone never asks for a node's credentials.
    expect(ipcMocks.getProfile).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Details for Server 3" }));

    const dialog = screen.getByRole("dialog", { name: "Node details" });
    await waitFor(() => expect(dialog).toHaveTextContent("24.0 KB"));
    expect(dialog).toHaveTextContent("Transportws");
    expect(dialog).toHaveTextContent("Securitytls");
    expect(ipcMocks.getProfile).toHaveBeenCalledWith("profile-3");
  });

  it("subscribes to live traffic only in the open node details", async () => {
    mockProfileList([makeProfile(0)]);
    renderProfiles();
    await screen.findByText("Server 0");
    act(() =>
      useRuntimeEventStore.setState({
        serverStatsByProfileId: {
          "profile-0": {
            dateNow: 20260101,
            indexId: "profile-0",
            todayDown: 4096,
            todayUp: 2048,
            totalDown: 8192,
            totalUp: 4096,
          },
        },
      }),
    );
    expect(screen.queryByText("4.0 KB")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Details for Server 0" }));
    expect(screen.getByRole("dialog")).toHaveTextContent("4.0 KB");
    act(() =>
      useRuntimeEventStore.setState({
        serverStatsByProfileId: {
          "profile-0": {
            dateNow: 20260101,
            indexId: "profile-0",
            todayDown: 16384,
            todayUp: 2048,
            totalDown: 16384,
            totalUp: 4096,
          },
        },
      }),
    );
    expect(screen.getByRole("dialog")).toHaveTextContent("16.0 KB");
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByText("16.0 KB")).not.toBeInTheDocument();
  });

  it("opens the same edit action from the visible card menu", async () => {
    mockProfileList([makeProfile(0)]);
    renderProfiles();
    await screen.findByText("Server 0");
    await userEvent.click(
      screen.getByRole("menuitem", { name: /Actions for Server 0/ }),
    );
    await userEvent.click(screen.getByRole("menuitem", { name: "Edit" }));
    expect(
      await screen.findByRole("dialog", { name: "Edit node" }),
    ).toBeInTheDocument();
  });

  it("keeps connection actions visible and distinguishes slow nodes from failed tests", async () => {
    const [chosen, other] = makeProfiles(2);
    other!.metrics.delayMs = 520;
    mockProfileList([chosen!, other!]);
    renderProfiles();
    await screen.findByText("Server 0");
    const [first, second] = screen.getAllByTestId("server-row");
    // Both nodes keep an explicit connection action.
    expect(
      within(first!).getByRole("button", { name: "Connect" }),
    ).not.toHaveClass("profile-node-action-idle");
    expect(
      within(second!).getByRole("button", { name: "Connect" }),
    ).not.toHaveClass("profile-node-action-idle");
    expect(second!.querySelector(".profile-node-latency")).toHaveAttribute(
      "data-tone",
      "fair",
    );
  });

  it("connects the requested card, guards repeated activation and marks the running node", async () => {
    mockProfileList(makeProfiles(2));
    let finish!: (status: RuntimeStatusResponse) => void;
    ipcMocks.connectActiveProfile.mockReturnValue(
      new Promise<RuntimeStatusResponse>((resolve) => {
        finish = resolve;
      }),
    );
    renderProfiles();
    await screen.findByText("Server 0");
    const target = within(screen.getAllByTestId("server-row")[1]!);
    fireEvent.click(target.getByRole("button", { name: "Connect" }));
    fireEvent.click(target.getByRole("button", { name: "Switching…" }));
    await waitFor(() =>
      expect(ipcMocks.connectActiveProfile).toHaveBeenCalledOnce(),
    );
    expect(ipcMocks.setActiveProfile).toHaveBeenCalledExactlyOnceWith(
      "profile-1",
    );
    expect(ipcMocks.setActiveProfile.mock.invocationCallOrder[0]).toBeLessThan(
      ipcMocks.connectActiveProfile.mock.invocationCallOrder[0]!,
    );
    // The first card is the chosen node, so while disconnected it offers to connect.
    expect(screen.getByRole("button", { name: "Connect" })).toBeDisabled();
    await act(async () =>
      finish({
        activeProfileId: "profile-1",
        activeTunBackend: null,
        mainPid: 42,
        prePid: null,
        state: "connected",
        connectedDurationMs: 0,
      }),
    );
    expect(
      await screen.findByRole("button", { name: "In use" }),
    ).toBeDisabled();
    expect(target.getByText("In use", { selector: ".node-card-state" })).toBeInTheDocument();
    expect(ipcMocks.restartCore).not.toHaveBeenCalled();
  });

  it("restarts when switching a connected node and keeps actual runtime distinct from default", async () => {
    mockProfileList(makeProfiles(2));
    useRuntimeEventStore.setState({
      coreState: {
        activeProfileId: "profile-1",
        activeTunBackend: null,
        mainPid: 42,
        prePid: null,
        state: "connected",
        connectedDurationMs: 0,
      },
    });
    renderProfiles();
    await screen.findByText("Server 0");
    const cards = screen.getAllByTestId("server-row");
    expect(within(cards[0]!).getByText("Selected")).toBeInTheDocument();
    expect(
      within(cards[1]!).getByRole("button", { name: "In use" }),
    ).toBeDisabled();
    await userEvent.click(
      within(cards[0]!).getByRole("button", { name: "Switch" }),
    );
    await waitFor(() => expect(ipcMocks.restartCore).toHaveBeenCalledOnce());
    await waitFor(() =>
      expect(
        within(cards[0]!).getByRole("button", { name: "In use" }),
      ).toBeDisabled(),
    );
    expect(ipcMocks.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("reports connection failure and allows retry", async () => {
    mockProfileList([makeProfile(0)]);
    ipcMocks.connectActiveProfile.mockRejectedValueOnce(
      new Error("Connection failed"),
    );
    renderProfiles();
    await screen.findByText("Server 0");
    await userEvent.click(screen.getByRole("button", { name: "Connect" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Connect" })).toBeEnabled(),
    );
    expect(
      useToastStore
        .getState()
        .toasts.some((toast) => toast.description === "Connection failed"),
    ).toBe(true);
    await userEvent.click(screen.getByRole("button", { name: "Connect" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "In use" })).toBeDisabled(),
    );
    expect(ipcMocks.connectActiveProfile).toHaveBeenCalledTimes(2);
  });

  it("keeps the operation guard across remounts until failed status reconciliation settles", async () => {
    mockProfileList([makeProfile(0)]);
    ipcMocks.connectActiveProfile.mockRejectedValueOnce(new Error("Connection failed"));
    let rejectStatus!: (error: Error) => void;
    ipcMocks.systemProxyStatus.mockImplementationOnce(() => new Promise((_resolve, reject) => {
      rejectStatus = reject;
    }));
    const view = renderProfiles();
    await userEvent.click(await screen.findByRole("button", { name: "Connect" }));
    await waitFor(() => expect(ipcMocks.systemProxyStatus).toHaveBeenCalledOnce());
    expect(useRuntimeActionStore.getState().switchingId).toBe("profile-0");

    view.unmount();
    renderProfiles();
    expect(await screen.findByRole("button", { name: "Switching…" })).toBeDisabled();
    await act(async () => rejectStatus(new Error("Proxy status unavailable")));
    await waitFor(() => expect(useRuntimeActionStore.getState().switchingId).toBeNull());
    expect(screen.getByRole("button", { name: "Connect" })).toBeEnabled();
    expect(useToastStore.getState().toasts.map((toast) => toast.description)).toEqual([
      "Connection failed", "Proxy status unavailable",
    ]);
    await userEvent.click(screen.getByRole("button", { name: "Connect" }));
    await waitFor(() => expect(ipcMocks.connectActiveProfile).toHaveBeenCalledTimes(2));
  });

  it.each(["connecting", "disconnecting", "cleanupPending"] as const)(
    "disables use during %s",
    async (state) => {
      mockProfileList([makeProfile(0)]);
      useRuntimeEventStore.setState({
        coreState: {
          activeProfileId: null,
          activeTunBackend: null,
          mainPid: null,
          prePid: null,
          state,
          connectedDurationMs: null,
        },
      });
      renderProfiles();
      await screen.findByText("Server 0");
      expect(screen.getByRole("button", { name: "Connect" })).toBeDisabled();
    },
  );

  it("opens missing-core recovery when using a card", async () => {
    mockProfileList([makeProfile(0)]);
    ipcMocks.connectActiveProfile.mockRejectedValue(
      new IpcCommandError({
        kind: {
          type: "missingCore",
          candidates: [],
          searchDir: "/cores",
          downloadUrl: "https://example.test/core",
        },
        message: "Core unavailable",
        subsystem: "runtime",
      }),
    );
    renderProfiles();
    await screen.findByText("Server 0");
    await userEvent.click(screen.getByRole("button", { name: "Connect" }));
    await waitFor(() =>
      expect(useRuntimeActionStore.getState().missingCore).toMatchObject({
        message: "Core unavailable",
      }),
    );
    expect(useToastStore.getState().toasts).toHaveLength(0);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Connect" })).toBeEnabled(),
    );
  });

  it.each([true, false])(
    "handles one-time authorization from a card (granted: %s)",
    async (granted) => {
      mockProfileList([makeProfile(0)]);
      ipcMocks.connectActiveProfile.mockRejectedValueOnce(
        new IpcCommandError({
          kind: { type: "elevationRequired" },
          message: "Authorization required",
          subsystem: "tun",
        }),
      );
      ipcMocks.tunRequestElevation.mockResolvedValue({
        elevationGranted: granted,
      });
      renderProfiles();
      await screen.findByText("Server 0");
      await userEvent.click(screen.getByRole("button", { name: "Connect" }));
      await waitFor(() =>
        expect(useRuntimeActionStore.getState().switchingId).toBeNull(),
      );
      expect(ipcMocks.tunRequestElevation).toHaveBeenCalledOnce();
      expect(ipcMocks.connectActiveProfile).toHaveBeenCalledTimes(
        granted ? 2 : 1,
      );
      if (!granted)
        expect(screen.getByRole("button", { name: "Connect" })).toBeEnabled();
    },
  );

  it("inherits a runtime operation started on another screen", async () => {
    useRuntimeActionStore.setState({ pendingAction: "connect" });
    mockProfileList([makeProfile(0)]);
    renderProfiles();
    await screen.findByText("Server 0");
    expect(screen.getByRole("button", { name: "Connect" })).toBeDisabled();
    act(() => useRuntimeActionStore.setState({ pendingAction: null }));
    expect(screen.getByRole("button", { name: "Connect" })).toBeEnabled();
    expect(ipcMocks.setActiveProfile).not.toHaveBeenCalled();
  });

  it("states how many stored profiles this build could not read", async () => {
    // Persistence hides rows it cannot decode so that one of them cannot take
    // the whole list down. Without this band the user only sees a list that is
    // mysteriously short, which is indistinguishable from data loss.
    mockProfileList(makeProfiles(2), 3);

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    expect(
      screen.getByText(
        "3 stored node(s) could not be read by this version and are hidden. They were most likely written by a newer build; updating VoyaVPN should show them again.",
      ),
    ).toBeInTheDocument();
  });

  // The backend picks how to measure (probe cores, or the connected tunnel), so
  // the page never waits for a connection.
  it("tests on macOS without a connection", async () => {
    mockProfileList(makeProfiles(2));
    useRuntimeEventStore.setState({ tun: macosTun });
    try {
      renderProfiles();
      await screen.findByText("Server 0");
      const testAll = within(screen.getByRole("toolbar")).getByRole("button", { name: "Test all" });
      expect(testAll).toBeEnabled();
      await userEvent.click(testAll);
      expect(ipcMocks.runSpeedtest).toHaveBeenCalledOnce();
    } finally {
      useRuntimeEventStore.setState({ tun: null });
    }
  });

  it("says nothing about unreadable profiles when every stored profile decoded", async () => {
    mockProfileList(makeProfiles(2));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    expect(
      screen.queryByText(/could not be read by this version/),
    ).not.toBeInTheDocument();
  });

  it("shows profile query errors instead of silently presenting an empty table", async () => {
    ipcMocks.listProfileSummaries.mockRejectedValue(new Error("profile list failed"));

    renderProfiles();

    expect(await screen.findByText("profile list failed")).toBeInTheDocument();
  });

  it("gathers adding and importing in one Add menu next to Update all subscriptions", async () => {
    mockProfileList([]);
    renderProfiles();
    const toolbar = within(screen.getByRole("toolbar"));
    expect(
      toolbar.getAllByRole("menuitem").map((item) => item.textContent),
    ).toEqual(["View", "Add"]);
    expect(
      toolbar.getAllByRole("button").map((button) => button.textContent),
    ).toEqual(["Test all", "Update all subscriptions"]);
    for (const name of ["Import", "More actions", "Subscriptions"]) {
      expect(toolbar.queryByRole("menuitem", { name })).not.toBeInTheDocument();
    }
    await userEvent.click(toolbar.getByRole("menuitem", { name: "Add" }));
    expect(
      within(screen.getByRole("menu"))
        .getAllByRole("menuitem")
        .map((item) => item.textContent),
    ).toEqual([
      "Paste links or subscription URLs",
      "Import from clipboard",
      "Scan screen",
      // The item carries a short hint under its name.
      "Add subscriptionName it and set auto-update and filters",
      "Enter a node manually",
      "New policy group",
    ]);
    await userEvent.click(screen.getByRole("menuitem", { name: "Add subscription" }));
    expect(
      await screen.findByRole("dialog", { name: "Add subscription" }),
    ).toBeVisible();
    await userEvent.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    await openImport();
    fireEvent.change(screen.getByLabelText("Import payload"), {
      target: { value: "vless://uuid@example.test:443#US" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Import" }));

    await waitFor(() =>
      expect(ipcMocks.importProfilesFromText).toHaveBeenCalledWith(
        "vless://uuid@example.test:443#US",
        null,
      ),
    );
    expect(ipcMocks.updateSubscriptions).not.toHaveBeenCalled();
  });

  it("updates a subscription pasted as a link right after importing it", async () => {
    mockProfileList([]);
    ipcMocks.importProfilesFromText.mockResolvedValue(
      makeImportResult({
        addedSubscriptionIds: ["sub-new"],
        lineIssues: [{ line: 1, code: { code: "subscriptionSourceAdded" } }],
      }),
    );
    ipcMocks.updateSubscriptions.mockResolvedValue({
      imported: 4,
      messages: [],
      skipped: 0,
      updated: 1,
    });
    renderProfiles();

    await openImport();
    fireEvent.change(screen.getByLabelText("Import payload"), {
      target: { value: "https://example.test/subscribe" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Import" }));

    await waitFor(() =>
      expect(ipcMocks.updateSubscriptions).toHaveBeenCalledWith("sub-new", true, null),
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
  });

  it("opens the paste dialog only after the method is chosen", async () => {
    const readText = mockClipboardReadText("vless://preview");
    mockProfileList([]);
    renderProfiles();
    const trigger = screen.getByRole("menuitem", { name: "Add" });
    await userEvent.click(trigger);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(readText).not.toHaveBeenCalled();
    expect(ipcMocks.scanScreenQr).not.toHaveBeenCalled();
    await userEvent.click(
      screen.getByRole("menuitem", { name: "Paste links or subscription URLs" }),
    );
    const dialog = await screen.findByRole("dialog", {
      name: "Add nodes or subscriptions",
    });
    expect(
      within(dialog).getByRole("button", { name: "Scan image" }),
    ).toBeVisible();
    expect(within(dialog).getByLabelText("Import payload")).toHaveAttribute(
      "placeholder",
      "vless://…\nhttps://example.com/subscription",
    );
    expect(readText).not.toHaveBeenCalled();
    expect(ipcMocks.importProfilesFromText).not.toHaveBeenCalled();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(trigger).toHaveFocus());
  });

  it("opens the node editor from the Add menu with the keyboard and restores focus", async () => {
    mockProfileList([]);
    renderProfiles();
    const trigger = screen.getByRole("menuitem", { name: "Add" });
    trigger.focus();
    await userEvent.keyboard("{Enter}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    const manual = screen.getByRole("menuitem", { name: "Enter a node manually" });
    manual.focus();
    await userEvent.keyboard("{Enter}");
    expect(
      await screen.findByRole("dialog", { name: "Add node" }),
    ).toBeVisible();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(trigger).toHaveFocus());
  });

  it("starts each Add subscription opening with a blank form and retains existing sources", async () => {
    mockProfileList([]);
    ipcMocks.listSubscriptions.mockResolvedValue([makeSubscription()]);
    renderProfiles();
    const trigger = screen.getByRole("menuitem", { name: "Add" });
    await userEvent.click(trigger);
    await userEvent.click(screen.getByRole("menuitem", { name: "Add subscription" }));
    fireEvent.change(screen.getByLabelText("Remarks"), {
      target: { value: "Draft" },
    });
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(trigger).toHaveFocus());
    await userEvent.click(trigger);
    await userEvent.click(screen.getByRole("menuitem", { name: "Add subscription" }));
    expect(await screen.findByLabelText("Remarks")).toHaveValue("");
    expect(screen.getByLabelText("URL")).toHaveValue("");
    await userEvent.click(screen.getByRole("button", { name: "Add and update" }));
    expect(screen.getByLabelText("Remarks")).toHaveFocus();
    expect(screen.getByText("Enter a subscription name.")).toBeInTheDocument();
    expect(ipcMocks.deleteSubscriptions).not.toHaveBeenCalled();
  });

  it("refreshes and selects imported profiles after dialog import", async () => {
    const importedProfile = makeProfile(7, {
      id: "profile-imported",
      remarks: "Imported node",
    });
    const secondImportedProfile = makeProfile(8, {
      id: "profile-imported-second",
      remarks: "Second imported node",
    });
    mockProfileListOnce([]);
    mockProfileList([importedProfile, secondImportedProfile]);
    ipcMocks.importProfilesFromText.mockResolvedValue(
      makeImportResult({
        imported: 2,
        importedProfileIds: ["profile-imported", "profile-imported-second"],
        parsed: 2,
      }),
    );

    renderProfiles();

    await openImport();
    fireEvent.change(screen.getByLabelText("Import payload"), {
      target: { value: "vless://uuid@example.test:443#Imported" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Import" }));

    expect(await screen.findByText("Imported node")).toBeInTheDocument();
    expect(screen.getByText("Imported 2 node(s).")).toBeInTheDocument();
  });

  it("refreshes and selects a profile imported from a scanned screen QR code", async () => {
    const importedProfile = makeProfile(8, {
      id: "profile-scanned",
      remarks: "Scanned node",
    });
    mockProfileListOnce([]);
    mockProfileListOnce([importedProfile]);
    ipcMocks.scanScreenQr.mockResolvedValue({
      message: null,
      source: "native",
      status: "found",
      texts: ["vless://uuid@example.test:443#Scanned"],
      failureReason: null,
    });
    ipcMocks.importProfilesFromText.mockResolvedValue(
      makeImportResult({
        imported: 1,
        importedProfileIds: ["profile-scanned"],
        parsed: 1,
      }),
    );

    renderProfiles();

    await openImport("Scan screen");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(ipcMocks.scanScreenQr).toHaveBeenCalledOnce();
    expect(ipcMocks.importProfilesFromText).toHaveBeenCalledWith("vless://uuid@example.test:443#Scanned", null);

    expect(await screen.findByText("Scanned node")).toBeInTheDocument();
    expect(screen.getByTestId("server-row")).toBeInTheDocument();
    expect(screen.getByText("Imported 1 node(s).")).toBeInTheDocument();
  });

  it("imports clipboard text directly and prevents duplicate submission", async () => {
    const clipboardText = "vless://uuid@example.test:443#US";
    const readText = mockClipboardReadText(`\n${clipboardText}\n`);
    const importedProfile = makeProfile(1, {
      id: "profile-clipboard",
      remarks: "Clipboard node",
    });
    let imported = false;
    let finishImport!: () => void;
    const pendingImport = new Promise<void>((resolve) => {
      finishImport = resolve;
    });
    ipcMocks.listProfileSummaries.mockImplementation(
      async (_subscription_id: string | null, filter: string | null) =>
        listing(imported && !filter ? [importedProfile] : []),
    );
    ipcMocks.importProfilesFromText.mockImplementation(async () => {
      await pendingImport;
      imported = true;
      return makeImportResult({
        imported: 1,
        importedProfileIds: ["profile-clipboard"],
        parsed: 1,
        removedDuplicates: 2,
        updated: 1,
        updatedProfileIds: ["profile-clipboard"],
      });
    });

    renderProfiles();

    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();

    await openImport("Import from clipboard");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    await waitFor(() => expect(readText).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(ipcMocks.importProfilesFromText).toHaveBeenCalledWith(
        clipboardText,
        null,
      ),
    );
    await userEvent.click(screen.getByRole("menuitem", { name: "Add" }));
    const clipboardItem = await screen.findByRole("menuitem", {
      name: "Import from clipboard",
    });
    expect(clipboardItem).toHaveAttribute("aria-disabled", "true");
    await userEvent.click(clipboardItem);
    expect(ipcMocks.importProfilesFromText).toHaveBeenCalledTimes(1);
    await userEvent.keyboard("{Escape}");
    await act(async () => {
      finishImport();
    });
    expect(await screen.findByText("Clipboard node")).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(screen.getByTestId("server-row")).toBeInTheDocument();
    expect(
      screen.getByText(
        "Imported 1 node(s). 1 existing node(s) refreshed. 2 duplicate(s) removed.",
      ),
    ).toBeInTheDocument();
  });

  it("does not import when clipboard text is empty", async () => {
    const readText = mockClipboardReadText(" \n ");
    mockProfileList([]);

    renderProfiles();

    await openImport("Import from clipboard");

    await waitFor(() => expect(readText).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("Clipboard is empty.")).toBeInTheDocument();
    expect(ipcMocks.importProfilesFromText).not.toHaveBeenCalled();
  });

  it("does not import when clipboard text read is unavailable", async () => {
    mockClipboardUnavailable();
    mockProfileList([]);

    renderProfiles();

    await openImport("Import from clipboard");

    expect(
      await screen.findByText(
        "Could not read text from the clipboard.",
      ),
    ).toBeInTheDocument();
    expect(ipcMocks.importProfilesFromText).not.toHaveBeenCalled();
  });

  it("keeps the QR dialog closed when share link export fails", async () => {
    mockProfileList([
      makeProfile(0, {
        remarks: "Export node",
      }),
    ]);
    ipcMocks.exportProfileShareLinks.mockRejectedValue(
      new Error("share export failed"),
    );

    renderProfiles();

    expect(await screen.findByText("Export node")).toBeInTheDocument();
    const menu = await openRowContextMenu();
    const exportMenu = await openContextSubmenu(menu, "Export");
    await userEvent.click(
      within(exportMenu).getByRole("menuitem", { name: "Show QR" }),
    );

    expect(await screen.findByText("share export failed")).toBeInTheDocument();
    expect(
      screen.queryByRole("dialog", { name: "Show QR" }),
    ).not.toBeInTheDocument();
    expect(ipcMocks.generateQrCode).not.toHaveBeenCalled();
  });

  it("shows QR generation errors without hiding the exported content", async () => {
    mockProfileList([makeProfile(0)]);
    ipcMocks.generateQrCode.mockRejectedValue(
      new Error("QR content is too large"),
    );

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    const menu = await openRowContextMenu();
    const exportMenu = await openContextSubmenu(menu, "Export");
    await userEvent.click(
      within(exportMenu).getByRole("menuitem", { name: "Show QR" }),
    );

    const dialog = await screen.findByRole("dialog", { name: "Show QR" });
    expect(
      await within(dialog).findByText("QR content is too large"),
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText("Content")).toHaveValue(
      "vless://profile-0@example.test:443",
    );
  });

  it("moves the context-menu row through the keyboard-accessible move submenu", async () => {
    mockProfileList(makeProfiles(3));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    const rows = screen.getAllByTestId("server-row");
    expect(rows[0]).not.toHaveAttribute("draggable");
    const menu = await openRowContextMenu();
    const moveMenu = await openContextSubmenu(menu, "Move");
    await userEvent.click(
      within(moveMenu).getByRole("menuitem", { name: "Move down" }),
    );

    await waitFor(() =>
      expect(ipcMocks.moveProfile).toHaveBeenCalledWith(
        null,
        "profile-0",
        MOVE_ACTIONS.Down,
        null,
      ),
    );
  });

  it("submits every protocol through the zod-backed profile dialog path", async () => {
    mockProfileList([]);

    renderProfiles();

    await openAddNode();

    await userEvent.click(screen.getByRole("combobox", { name: "Protocol" }));
    const protocolOptions = within(
      await screen.findByRole("listbox"),
    ).getAllByRole("option");
    const protocolLabels = [
      "VMess",
      "Shadowsocks",
      "SOCKS",
      "VLESS",
      "Trojan",
      "Hysteria2",
      "TUIC",
      "WireGuard",
      "HTTP",
      "AnyTLS",
      "Naive",
    ];
    expect(protocolOptions).toHaveLength(protocolLabels.length);
    protocolLabels.forEach((label) => {
      expect(
        screen.getByRole("option", {
          name: new RegExp(`^${escapeRegExp(label)}`),
        }),
      ).toBeInTheDocument();
    });

    await userEvent.click(screen.getByRole("option", { name: /^WireGuard/ }));
    expect(await screen.findByLabelText("Peer public key")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Remarks"), {
      target: { value: "WireGuard test" },
    });
    fireEvent.change(screen.getByLabelText("Address"), {
      target: { value: "wg.example.test" },
    });
    fireEvent.change(screen.getByLabelText("Private key"), {
      target: { value: "private-key" },
    });
    fireEvent.change(screen.getByLabelText("Peer public key"), {
      target: { value: "peer-key" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Save/ }));

    await waitFor(() =>
      expect(ipcMocks.saveProfile).toHaveBeenCalledWith(
        expect.objectContaining({
          protocol: expect.objectContaining({
            kind: "wireGuard",
            peerPublicKey: "peer-key",
            privateKey: "private-key",
            server: { address: "wg.example.test", port: 443 },
          }),
          remarks: "WireGuard test",
        }),
      ),
    );
  });

  it("keeps the editor open with its edits when the backend rejects the save", async () => {
    mockProfileList([]);
    ipcMocks.saveProfile.mockRejectedValue(
      new Error("profile address is already used"),
    );

    renderProfiles();

    await openAddNode();
    fireEvent.change(await screen.findByLabelText("Remarks"), {
      target: { value: "Rejected node" },
    });
    fireEvent.change(screen.getByLabelText("Address"), {
      target: { value: "node.example.test" },
    });
    fireEvent.change(screen.getByLabelText("UUID"), {
      target: { value: "uuid-rejected" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Save/ }));

    await waitFor(() => expect(ipcMocks.saveProfile).toHaveBeenCalled());
    const dialog = await screen.findByRole("dialog", { name: "Add node" });
    expect(
      await within(dialog).findByText("profile address is already used"),
    ).toBeInTheDocument();
    // The form remounts whenever the dialog toggles, so staying open is what
    // preserves the values the user already typed.
    expect(within(dialog).getByLabelText("Remarks")).toHaveValue(
      "Rejected node",
    );
    expect(within(dialog).getByLabelText("UUID")).toHaveValue("uuid-rejected");
  });

  it("shows the required credential error instead of silently refusing to save", async () => {
    mockProfileList([]);

    renderProfiles();

    await openAddNode();
    fireEvent.change(await screen.findByLabelText("Remarks"), {
      target: { value: "Missing UUID" },
    });
    fireEvent.change(screen.getByLabelText("Address"), {
      target: { value: "node.example.test" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Save/ }));

    expect(
      await screen.findByText("password or ID is required"),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("UUID")).toHaveAttribute(
      "aria-invalid",
      "true",
    );
    expect(ipcMocks.saveProfile).not.toHaveBeenCalled();
  });

  it("saves the TUIC uuid and password into the fields the contract names", async () => {
    mockProfileList([]);

    renderProfiles();

    await openAddNode();
    await selectComboboxOption("Protocol", "TUIC");
    expect(
      await screen.findByLabelText("Congestion control"),
    ).toBeInTheDocument();
    // `insecureConcurrency` belongs to Naive, not TUIC; rendering it here would
    // silently discard whatever the user typed.
    expect(
      screen.queryByLabelText("Insecure concurrency"),
    ).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Remarks"), {
      target: { value: "TUIC node" },
    });
    fireEvent.change(screen.getByLabelText("Address"), {
      target: { value: "tuic.example.test" },
    });
    fireEvent.change(screen.getByLabelText("UUID"), {
      target: { value: "uuid-tuic" },
    });
    fireEvent.change(screen.getByLabelText("Password"), {
      target: { value: "tuic-secret" },
    });
    fireEvent.change(screen.getByLabelText("Congestion control"), {
      target: { value: "bbr" },
    });
    fireEvent.click(screen.getByRole("button", { name: /Save/ }));

    await waitFor(() =>
      expect(ipcMocks.saveProfile).toHaveBeenCalledWith(
        expect.objectContaining({
          protocol: {
            congestionControl: "bbr",
            kind: "tuic",
            password: "tuic-secret",
            server: { address: "tuic.example.test", port: 443 },
            uuid: "uuid-tuic",
          },
          remarks: "TUIC node",
        }),
      ),
    );
  });

  it("edits every Naive contract field from the protocol panel", async () => {
    const user = userEvent.setup();
    mockProfileList([]);

    renderProfiles();

    await openAddNode();
    await selectComboboxOption("Protocol", "Naive");
    expect(
      await screen.findByLabelText("Insecure concurrency"),
    ).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Remarks"), {
      target: { value: "Naive node" },
    });
    fireEvent.change(screen.getByLabelText("Address"), {
      target: { value: "naive.example.test" },
    });
    fireEvent.change(screen.getByLabelText("Username"), {
      target: { value: "naive-user" },
    });
    fireEvent.change(screen.getByLabelText("Password"), {
      target: { value: "naive-secret" },
    });
    fireEvent.change(screen.getByLabelText("Congestion control"), {
      target: { value: "bbr" },
    });
    fireEvent.change(screen.getByLabelText("Insecure concurrency"), {
      target: { value: "4" },
    });
    await user.click(screen.getByRole("checkbox", { name: "QUIC" }));
    await user.click(screen.getByRole("checkbox", { name: "UDP over TCP" }));
    fireEvent.click(screen.getByRole("button", { name: /Save/ }));

    await waitFor(() =>
      expect(ipcMocks.saveProfile).toHaveBeenCalledWith(
        expect.objectContaining({
          protocol: {
            congestionControl: "bbr",
            insecureConcurrency: 4,
            kind: "naive",
            password: "naive-secret",
            quic: true,
            server: { address: "naive.example.test", port: 443 },
            udpOverTcp: true,
            username: "naive-user",
          },
          remarks: "Naive node",
        }),
      ),
    );
  });

  it("keeps the host and path of a raw TCP transport through an editor round-trip", async () => {
    mockProfileList([
      makeProfile(0, {
        remarks: "Obfuscated node",
        transport: {
          header: "http",
          host: "cdn.example.test",
          kind: "tcp",
          path: "/obfs",
        },
      }),
    ]);

    renderProfiles();

    expect(await screen.findByText("Obfuscated node")).toBeInTheDocument();
    const menu = await openRowContextMenu();
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Edit" }));

    const dialog = await screen.findByRole("dialog", { name: "Edit node" });
    expect(within(dialog).getByLabelText("Host")).toHaveValue(
      "cdn.example.test",
    );
    expect(within(dialog).getByLabelText("Path")).toHaveValue("/obfs");
    fireEvent.change(within(dialog).getByLabelText("Remarks"), {
      target: { value: "Renamed node" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: /Save/ }));

    await waitFor(() =>
      expect(ipcMocks.saveProfile).toHaveBeenCalledWith(
        expect.objectContaining({
          remarks: "Renamed node",
          transport: {
            header: "http",
            host: "cdn.example.test",
            kind: "tcp",
            path: "/obfs",
          },
        }),
      ),
    );
  });

  it("gives every editor field a real id in a locale whose labels have no ASCII letters", async () => {
    mockProfileList([]);

    // Ids used to be slugified from the translated label, so "备注" collapsed to
    // the empty string and every input in the dialog shared `id=""`.
    await withLocale("zh-Hans", async () => {
      renderProfiles();

      await userEvent.click(screen.getByRole("menuitem", { name: "添加" }));
      await userEvent.click(screen.getByRole("menuitem", { name: "手动填写节点" }));
      const dialog = await screen.findByRole("dialog", { name: "新增节点" });
      const remarks = within(dialog).getByLabelText("备注");
      const address = within(dialog).getByLabelText("地址");

      expect(remarks.id).not.toBe("");
      expect(address.id).not.toBe("");
      expect(address.id).not.toBe(remarks.id);
      expect(dialog.querySelectorAll('[id=""]')).toHaveLength(0);
    });
  });

  it("renders required-field errors through the locale system", async () => {
    mockProfileList([]);

    await withLocale("zh-Hans", async () => {
      renderProfiles();

      await userEvent.click(screen.getByRole("menuitem", { name: "添加" }));
      await userEvent.click(screen.getByRole("menuitem", { name: "手动填写节点" }));
      fireEvent.click(await screen.findByRole("button", { name: /保存/ }));

      // The zod schema carries codes; the visible sentence comes from the locale.
      expect(await screen.findByText("请填写备注")).toBeInTheDocument();
      expect(screen.getByText("请填写地址")).toBeInTheDocument();
      expect(screen.getByText("请填写密码或 ID")).toBeInTheDocument();
      expect(ipcMocks.saveProfile).not.toHaveBeenCalled();
    });
  });

  it("localizes the import summary banner", async () => {
    mockClipboardReadText("vless://uuid@example.test:443#US");
    mockProfileList([]);
    ipcMocks.importProfilesFromText.mockResolvedValue(
      makeImportResult({ failed: 2, imported: 3, skipped: 1, importedProfileIds: ["a", "b", "c"] }),
    );

    await withLocale("zh-Hans", async () => {
      renderProfiles();

      await userEvent.click(screen.getByRole("menuitem", { name: "添加" }));
      await userEvent.click(
        await screen.findByRole("menuitem", { name: "从剪贴板导入" }),
      );

      expect(
        await screen.findByText(
          "已导入 3 个节点。 已跳过 1 个。 2 个解析失败。",
        ),
      ).toBeInTheDocument();
    });
  });

  it("offers only the direct latency action in the row menu", async () => {
    mockProfileList(makeProfiles(1));
    renderProfiles();
    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    const menu = await openRowContextMenu();
    const latencyItem = within(menu).getByRole("menuitem", { name: "Ping" });
    expect(latencyItem).not.toHaveAttribute("aria-haspopup");
    for (const name of ["TCP", "UDP", "Speed", "Mixed", "Speedtest"]) {
      expect(
        within(menu).queryByRole("menuitem", { name }),
      ).not.toBeInTheDocument();
    }
  });

  it("renders the shared export entries through the context-menu primitives", async () => {
    mockProfileList(makeProfiles(1));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    const menu = await openRowContextMenu();
    const exportMenu = await openContextSubmenu(menu, "Export");

    // The row menu and the group card map the same descriptor list.
    expect(
      within(exportMenu)
        .getAllByRole("menuitem")
        .map((item) => item.textContent),
    ).toEqual(["Share links", "Show QR"]);
  });
});

// Every field of the generated `ImportProfilesResult`; overriding only what a
// test cares about keeps the mocks from drifting away from the contract.
function makeImportResult(
  overrides: Partial<ImportProfilesResult> = {},
): ImportProfilesResult {
  return {
    deduped: 0,
    discardedNodeOverrides: 0,
    failed: 0,
    filtered: 0,
    imported: 0,
    importedProfileIds: [],
    lineIssues: [],
    addedSubscriptionIds: [],
    parsed: 0,
    removedDuplicates: 0,
    removedExisting: 0,
    skipped: 0,
    subscriptionId: null,
    updated: 0,
    updatedProfileIds: [],
    ...overrides,
  };
}

function makeProfiles(count: number) {
  return Array.from({ length: count }, (_, index) => makeProfile(index));
}

/** Every node a test built, in full, for `getProfile` to answer with. */
const detailsById = new Map<string, ProfileDetails>();

function makeProfile(index: number, overrides: Partial<Profile> = {}) {
  const details = makeProfileDetailsFixture(index, overrides);
  detailsById.set(details.profile.id, details);
  return toProfileSummaryEntry(details);
}

function makeSubscription() {
  return {
    additionalUrl: "",
    converterTarget: null,
    enabled: true,
    filter: null,
    id: "sub-1",
    remarks: "Fixture",
    sort: 1,
    url: "https://example.test/sub",
    userAgent: "",
  };
}

async function openAddNode() {
  await userEvent.click(screen.getByRole("menuitem", { name: "Add" }));
  await userEvent.click(screen.getByRole("menuitem", { name: "Enter a node manually" }));
}

async function openImport(method = "Paste links or subscription URLs") {
  await userEvent.click(screen.getByRole("menuitem", { name: "Add" }));
  await userEvent.click(await screen.findByRole("menuitem", { name: method }));
}

const macosTun: TunStatus = {
  allowEnableTun: true,
  backend: "macosPacketTunnel",
  elevationGranted: false,
  enabled: true,
  expectedProviderPath: null,
  lastProviderError: null,
  nativeComponentReady: true,
  needsServiceInstall: false,
  needsVpnPermission: false,
  preflight: { notes: [], platform: "macos", routeRestoreNote: "", state: "ready", windowsCleanupDevices: [] },
  providerPathMismatch: false,
  providerState: "stopped",
  requiresElevation: false,
  resolvedProviderPath: null,
  restoreOnDisconnect: true,
};
