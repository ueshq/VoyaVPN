import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, vi } from "vitest";

import type { Profile } from "@/ipc/bindings";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useProfileColumnsStore } from "@/stores/profile-columns-store";
import { makeProfileFixture } from "@/test/profile-fixture";

import { MOVE_ACTIONS, SPEED_ACTIONS } from "./profile-constants";
import { ProfilesScreen } from "./server-table";
import { applyLiveUpdates } from "./server-table-live-updates";

const ipcMocks = vi.hoisted(() => ({
  dedupeProfiles: vi.fn(),
  deleteSubscriptions: vi.fn(),
  deleteProfiles: vi.fn(),
  exportProfileShareLinks: vi.fn(),
  generateQrCode: vi.fn(),
  importProfilesFromText: vi.fn(),
  listGroupChildCandidates: vi.fn(),
  listProfiles: vi.fn(),
  listSubscriptionMetadata: vi.fn(() => Promise.resolve([])),
  listSubscriptions: vi.fn(),
  moveProfile: vi.fn(),
  previewGroupProfile: vi.fn(),
  cancelSpeedtest: vi.fn(),
  runSpeedtest: vi.fn(),
  scanScreenQr: vi.fn(),
  saveGroupProfile: vi.fn(),
  saveProfile: vi.fn(),
  saveSubscription: vi.fn(),
  setActiveProfile: vi.fn(),
  sortProfiles: vi.fn(),
  updateSubscriptions: vi.fn(),
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
    Object.values(ipcMocks).forEach((mock) => {
      if ("mockReset" in mock) {
        mock.mockReset();
      }
    });
    // Column visibility persists to localStorage, so reset it between tests to
    // keep the default-column expectations independent of prior toggles.
    useProfileColumnsStore.getState().resetColumnVisibility();
    useRuntimeEventStore.setState({
      serverStatsByProfileId: {},
      speedtestResultsByProfileId: {},
      speedtestRunning: false,
    });
    ipcMocks.dedupeProfiles.mockResolvedValue({ kept: 0, removedIndexIds: [], total: 0 });
    ipcMocks.deleteSubscriptions.mockResolvedValue(1);
    ipcMocks.deleteProfiles.mockResolvedValue(1);
    ipcMocks.exportProfileShareLinks.mockImplementation(async (indexIds: string[]) => ({
      count: indexIds.length,
      format: "shareLinks",
      text: indexIds.map((indexId) => `vless://${indexId}@example.test:443`).join("\n"),
    }));
    ipcMocks.generateQrCode.mockResolvedValue({ mimeType: "image/svg+xml", svg: "<svg />" });
    ipcMocks.importProfilesFromText.mockResolvedValue({ imported: 1, importedProfileIds: ["profile-new"], removedExisting: 0, skipped: 0, subscriptionId: null });
    ipcMocks.listGroupChildCandidates.mockResolvedValue([]);
    ipcMocks.listSubscriptions.mockResolvedValue([]);
    ipcMocks.moveProfile.mockResolvedValue([]);
    ipcMocks.previewGroupProfile.mockResolvedValue({
      validation: { childProfileIds: [], errors: [], valid: true, warnings: [] },
      singboxRoutes: [],
    });
    ipcMocks.cancelSpeedtest.mockResolvedValue({ running: false });
    ipcMocks.runSpeedtest.mockResolvedValue({
      action: SPEED_ACTIONS.Download,
      cancelled: false,
      completedCount: 0,
      results: [],
      selectedCount: 0,
    });
    ipcMocks.scanScreenQr.mockResolvedValue({
      message: null,
      source: "screen",
      status: "unavailable",
      text: null,
    });
    ipcMocks.saveGroupProfile.mockImplementation(async (profile: Profile) => makeProfile(100, profile));
    ipcMocks.saveProfile.mockImplementation(async (profile: Profile) => makeProfile(99, profile));
    ipcMocks.saveSubscription.mockResolvedValue(makeSubscription());
    ipcMocks.setActiveProfile.mockImplementation(async (profileId: string) => makeProfile(0, { id: profileId }));
    ipcMocks.sortProfiles.mockResolvedValue([]);
    ipcMocks.updateSubscriptions.mockResolvedValue({ imported: 0, messages: [], removedExisting: 0, skipped: 0, updated: 0 });
  });

  it("keeps a 5k row profile list virtualized", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(5000));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    expect(screen.getByRole("table")).toHaveAttribute("aria-rowcount", "5001");
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
          profile.profile.id,
          {
            dateNow: profile.traffic.date ?? 0,
            indexId: profile.profile.id,
            todayDown: index * 2048 + tick,
            todayUp: index * 1024 + tick,
            totalDown: index * 8192 + tick,
            totalUp: index * 4096 + tick,
          },
        ]),
      );

      updated = applyLiveUpdates(profiles, stats, {});
    }

    expect(performance.now() - startedAt).toBeLessThan(1000);
    expect(updated).toHaveLength(500);
    expect(updated[499].traffic.todayDownload).toBe(499 * 2048 + 59);
  });

  it("shows speedtest status messages even when a previous speed value exists", async () => {
    const profile = makeProfile(0);
    profile.metrics = {
      ...profile.metrics,
      delayMs: -1,
      ipInfo: "Skipped",
      message: "request timed out",
      speedBytesPerSecond: 2048,
    };
    ipcMocks.listProfiles.mockResolvedValue([profile]);

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    expect(screen.queryByRole("columnheader", { name: "Speed" })).not.toBeInTheDocument();
    const menu = await openRowContextMenu();
    const speedMenu = await openContextSubmenu(menu, "Speedtest");
    await userEvent.click(within(speedMenu).getByRole("menuitem", { name: "Speed" }));

    expect(await screen.findByRole("columnheader", { name: "Speed" })).toBeInTheDocument();
    expect(await screen.findByText("request timed out")).toBeInTheDocument();
    expect(screen.queryByText("2.0 KB/s")).not.toBeInTheDocument();
  });

  it("removes multi-select and copy while keeping row actions and sorting", async () => {
    const profiles = makeProfiles(3);
    ipcMocks.listProfiles.mockResolvedValue(profiles);

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    const rows = screen.getAllByTestId("server-row");
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Copy" })).not.toBeInTheDocument();

    await userEvent.click(rows[0]!);
    fireEvent.click(rows[1]!, { ctrlKey: true });
    expect(rows[0]).toHaveAttribute("aria-selected", "false");
    expect(rows[1]).toHaveAttribute("aria-selected", "true");
    expect(rows[2]).toHaveAttribute("aria-selected", "false");
    expect(screen.queryByRole("button", { name: /Activate/ })).not.toBeInTheDocument();

    const menu = await openRowContextMenu(0);
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    const confirm = await screen.findByRole("alertdialog");
    fireEvent.click(within(confirm).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    await userEvent.click(screen.getByRole("button", { name: "Remarks" }));

    expect(ipcMocks.setActiveProfile).not.toHaveBeenCalled();
    expect(ipcMocks.deleteProfiles).toHaveBeenCalledWith(["profile-0"]);
    expect(ipcMocks.sortProfiles).toHaveBeenCalledWith(null, "remarks", true);
  });

  it("opens the targeted profile editor from the row context menu", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(2));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    const menu = await openRowContextMenu(1);
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Edit" }));

    const dialog = await screen.findByRole("dialog", { name: "Edit profile" });
    expect(within(dialog).getByLabelText("Remarks")).toHaveValue("Server 1");
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

    const speedButton = screen.getByRole("button", { name: "Bulk speedtest" });
    await userEvent.click(speedButton);

    await waitFor(() => expect(speedButton).toBeDisabled());
    expect(ipcMocks.runSpeedtest).toHaveBeenCalledWith({
      kind: SPEED_ACTIONS.Latency,
      target: { scope: "all" },
    });

    rejectSpeedtest(new Error("boom"));

    await waitFor(() => expect(speedButton).toBeEnabled());
  });

  it("runs a row speedtest only for the context-menu target", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(2));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    const menu = await openRowContextMenu();
    const speedMenu = await openContextSubmenu(menu, "Speedtest");
    await userEvent.click(within(speedMenu).getByRole("menuitem", { name: "Real" }));

    expect(ipcMocks.runSpeedtest).toHaveBeenCalledWith({
      kind: SPEED_ACTIONS.Latency,
      target: { scope: "profiles", profileIds: ["profile-0"] },
    });
  });

  it("runs alternate batch speedtests against all profiles", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(2));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("menuitem", { name: "More speed tests" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "TCP" }));

    expect(ipcMocks.runSpeedtest).toHaveBeenCalledWith({
      kind: SPEED_ACTIONS.TcpConnect,
      target: { scope: "all" },
    });
  });

  it("reflects an already running speedtest from the runtime store", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(1));
    useRuntimeEventStore.setState({ speedtestRunning: true });

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    await waitFor(() => expect(screen.getByRole("button", { name: "Bulk speedtest" })).toBeDisabled());

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

    const menu = await openRowContextMenu();
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Delete" }));

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

  it("shows profile query errors instead of silently presenting an empty table", async () => {
    ipcMocks.listProfiles.mockRejectedValue(new Error("profile list failed"));

    renderProfiles();

    expect(await screen.findByText("profile list failed")).toBeInTheDocument();
  });

  it("keeps import and subscription management without a duplicate update-all action", async () => {
    ipcMocks.listProfiles.mockResolvedValue([]);

    renderProfiles();

    // Import and subscription management live in the toolbar overflow menu;
    // update-all is intentionally available only inside subscription management.
    await userEvent.click(await screen.findByRole("menuitem", { name: "More actions" }));
    expect(screen.queryByRole("menuitem", { name: "Update subs" })).not.toBeInTheDocument();
    await userEvent.click(await screen.findByRole("menuitem", { name: "Import" }));
    fireEvent.change(screen.getByLabelText("Import payload"), {
      target: { value: "vless://uuid@example.test:443#US" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Import payload" }));

    await waitFor(() =>
      expect(ipcMocks.importProfilesFromText).toHaveBeenCalledWith(
        "vless://uuid@example.test:443#US",
        null,
      ),
    );
    expect(ipcMocks.updateSubscriptions).not.toHaveBeenCalled();
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
    ipcMocks.listProfiles
      .mockResolvedValueOnce([])
      .mockResolvedValue([importedProfile, secondImportedProfile]);
    ipcMocks.importProfilesFromText.mockResolvedValue({
      deduped: 0,
      failed: 0,
      filtered: 0,
      imported: 2,
      importedProfileIds: ["profile-imported", "profile-imported-second"],
      messages: [],
      parsed: 2,
      removedExisting: 0,
      skipped: 0,
      subscriptionId: null,
    });

    renderProfiles();

    await userEvent.click(await screen.findByRole("menuitem", { name: "More actions" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Import" }));
    fireEvent.change(screen.getByLabelText("Import payload"), {
      target: { value: "vless://uuid@example.test:443#Imported" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Import payload" }));

    expect(await screen.findByText("Imported node")).toBeInTheDocument();
    const rows = screen.getAllByTestId("server-row");
    expect(rows[0]).toHaveAttribute("aria-selected", "true");
    expect(rows[1]).toHaveAttribute("aria-selected", "false");
    expect(screen.getByText("Imported 2 profiles.")).toBeInTheDocument();
  });

  it("refreshes and selects a profile imported from a scanned screen QR code", async () => {
    const importedProfile = makeProfile(8, {
      id: "profile-scanned",
      remarks: "Scanned node",
    });
    ipcMocks.listProfiles
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([importedProfile]);
    ipcMocks.scanScreenQr.mockResolvedValue({
      message: null,
      source: "native",
      status: "found",
      text: "vless://uuid@example.test:443#Scanned",
    });
    ipcMocks.importProfilesFromText.mockResolvedValue({
      deduped: 0,
      failed: 0,
      filtered: 0,
      imported: 1,
      importedProfileIds: ["profile-scanned"],
      messages: [],
      parsed: 1,
      removedExisting: 0,
      skipped: 0,
      subscriptionId: null,
    });

    renderProfiles();

    await userEvent.click(await screen.findByRole("menuitem", { name: "More actions" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Import" }));
    await userEvent.click(await screen.findByRole("button", { name: "Screen" }));
    expect(await screen.findByLabelText("Import payload")).toHaveValue(
      "vless://uuid@example.test:443#Scanned",
    );
    expect(ipcMocks.importProfilesFromText).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: "Import payload" }));

    expect(await screen.findByText("Scanned node")).toBeInTheDocument();
    expect(screen.getByTestId("server-row")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("Imported 1 profile.")).toBeInTheDocument();
  });

  it("imports profiles directly from clipboard text", async () => {
    const clipboardText = "vless://uuid@example.test:443#US";
    const readText = mockClipboardReadText(`\n${clipboardText}\n`);
    const importedProfile = makeProfile(1, {
      id: "profile-clipboard",
      remarks: "Clipboard node",
    });
    let imported = false;
    ipcMocks.listProfiles.mockImplementation(async (_subscription_id: string | null, filter: string | null) =>
      imported && !filter ? [importedProfile] : [],
    );
    ipcMocks.importProfilesFromText.mockImplementation(async () => {
      imported = true;
      return {
        deduped: 0,
        failed: 0,
        filtered: 0,
        imported: 1,
        importedProfileIds: ["profile-clipboard"],
        messages: [],
        parsed: 1,
        removedExisting: 0,
        removedDuplicates: 2,
        skipped: 0,
        subscriptionId: null,
        updated: 1,
        updatedProfileIds: ["profile-clipboard"],
      };
    });

    renderProfiles();

    const filterInput = await screen.findByRole("searchbox", { name: "Filter profiles" });
    fireEvent.change(filterInput, { target: { value: "hidden" } });

    await userEvent.click(await screen.findByRole("menuitem", { name: "More actions" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Import from clipboard" }));

    await waitFor(() => expect(readText).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(ipcMocks.importProfilesFromText).toHaveBeenCalledWith(clipboardText, null),
    );
    expect(await screen.findByText("Clipboard node")).toBeInTheDocument();
    expect(filterInput).toHaveValue("");
    expect(screen.getByTestId("server-row")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByText("Imported 1 profile. 1 updated. 2 duplicates removed.")).toBeInTheDocument();
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

  it("batch exports every profile even when the list is filtered", async () => {
    const profiles = makeProfiles(2);
    ipcMocks.listProfiles.mockImplementation(async (_subscriptionId: string | null, filter: string | null) =>
      filter ? [profiles[1]!] : profiles,
    );

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("searchbox", { name: "Filter profiles" }), {
      target: { value: "Server 1" },
    });
    await waitFor(() => expect(ipcMocks.listProfiles).toHaveBeenCalledWith(null, "Server 1"));

    await userEvent.click(screen.getByRole("menuitem", { name: "Bulk export" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Show QR" }));

    const expectedContent =
      "vless://profile-0@example.test:443\nvless://profile-1@example.test:443";
    expect(ipcMocks.exportProfileShareLinks).toHaveBeenCalledWith(["profile-0", "profile-1"]);
    expect(ipcMocks.listProfiles).toHaveBeenLastCalledWith(null, null);
    await waitFor(() => expect(ipcMocks.generateQrCode).toHaveBeenCalledWith(expectedContent));

    const dialog = await screen.findByRole("dialog", { name: "Show QR" });
    expect(within(dialog).getByLabelText("Content")).toHaveValue(expectedContent);
    expect(within(dialog).getByLabelText("Content")).toHaveAttribute("readonly");
    expect(within(dialog).getByAltText("Generated QR code")).toBeInTheDocument();
  });

  it("keeps the QR dialog closed when the context-menu profile cannot export a share link", async () => {
    ipcMocks.listProfiles.mockResolvedValue([
      makeProfile(0, {
        protocol: {
          childProfileIds: [],
          filter: null,
          kind: "policyGroup",
          sourceSubscriptionId: null,
          strategy: "leastPing",
        },
        remarks: "Policy group",
      }),
    ]);
    ipcMocks.exportProfileShareLinks.mockRejectedValue(
      new Error("share export does not support policy groups"),
    );

    renderProfiles();

    expect(await screen.findByText("Policy group")).toBeInTheDocument();
    const menu = await openRowContextMenu();
    const exportMenu = await openContextSubmenu(menu, "Export");
    await userEvent.click(within(exportMenu).getByRole("menuitem", { name: "Show QR" }));

    expect(await screen.findByText("share export does not support policy groups")).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Show QR" })).not.toBeInTheDocument();
    expect(ipcMocks.generateQrCode).not.toHaveBeenCalled();
  });

  it("shows QR generation errors without hiding the exported content", async () => {
    ipcMocks.listProfiles.mockResolvedValue([makeProfile(0)]);
    ipcMocks.generateQrCode.mockRejectedValue(new Error("QR content is too large"));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    const menu = await openRowContextMenu();
    const exportMenu = await openContextSubmenu(menu, "Export");
    await userEvent.click(within(exportMenu).getByRole("menuitem", { name: "Show QR" }));

    const dialog = await screen.findByRole("dialog", { name: "Show QR" });
    expect(await within(dialog).findByText("QR content is too large")).toBeInTheDocument();
    expect(within(dialog).getByLabelText("Content")).toHaveValue(
      "vless://profile-0@example.test:443",
    );
  });

  it("moves the context-menu row through the keyboard-accessible move submenu", async () => {
    ipcMocks.listProfiles.mockResolvedValue(makeProfiles(3));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    const rows = screen.getAllByTestId("server-row");
    expect(rows[0]).not.toHaveAttribute("draggable");
    const menu = await openRowContextMenu();
    const moveMenu = await openContextSubmenu(menu, "Move");
    await userEvent.click(within(moveMenu).getByRole("menuitem", { name: "Move down" }));

    await waitFor(() =>
      expect(ipcMocks.moveProfile).toHaveBeenCalledWith(null, "profile-0", MOVE_ACTIONS.Down, null),
    );
  });

  it("submits every protocol through the zod-backed profile dialog path", async () => {
    ipcMocks.listProfiles.mockResolvedValue([]);

    renderProfiles();

    fireEvent.click(screen.getByRole("button", { name: "Add" }));

    await userEvent.click(screen.getByRole("combobox", { name: "Protocol" }));
    const protocolOptions = within(await screen.findByRole("listbox")).getAllByRole("option");
    const protocolLabels = [
      "VMess", "Custom", "Shadowsocks", "SOCKS", "VLESS", "Trojan", "Hysteria2",
      "TUIC", "WireGuard", "HTTP", "AnyTLS", "Naive", "Policy Group", "Proxy Chain",
    ];
    expect(protocolOptions).toHaveLength(protocolLabels.length);
    protocolLabels.forEach((label) => {
      expect(screen.getByRole("option", { name: new RegExp(`^${escapeRegExp(label)}`) })).toBeInTheDocument();
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

  it("builds a policy group with child picker and generator preview", async () => {
    const user = userEvent.setup();

    ipcMocks.listProfiles.mockResolvedValue([]);
    ipcMocks.listGroupChildCandidates.mockResolvedValue([
      {
        address: "a.example.test",
        isGroup: false,
        profileId: "leaf-a",
        protocol: "vless",
        reason: null,
        remarks: "Leaf A",
        selectable: true,
        subscriptionId: "",
      },
      {
        address: "chain",
        isGroup: true,
        profileId: "chain-a",
        protocol: "proxyChain",
        reason: null,
        remarks: "Chain A",
        selectable: true,
        subscriptionId: "",
      },
    ]);
    ipcMocks.previewGroupProfile.mockResolvedValue({
      validation: {
        childProfileIds: ["leaf-a", "chain-a"],
        errors: [],
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
    expect(await screen.findByText("Generated routes")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Save/ }));

    await waitFor(() =>
      expect(ipcMocks.saveGroupProfile).toHaveBeenCalledWith(
        expect.objectContaining({
          protocol: expect.objectContaining({
            childProfileIds: ["leaf-a", "chain-a"],
            kind: "policyGroup",
          }),
          remarks: "Mixed policy",
        }),
      ),
    );
  }, 10_000);
});

function makeProfiles(count: number) {
  return Array.from({ length: count }, (_, index) => makeProfile(index));
}

function makeProfile(index: number, overrides: Partial<Profile> = {}) {
  return makeProfileFixture(index, overrides);
}

function makeSubscription() {
  return {
    additionalUrl: "",
    converterTarget: null,
    enabled: true,
    filter: null,
    id: "sub-1",
    preSocksPort: null,
    remarks: "Fixture",
    sort: 1,
    url: "https://example.test/sub",
    userAgent: "",
  };
}
