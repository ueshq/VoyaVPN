import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, vi } from "vitest";

import { changeLocale } from "@voya/i18n";

import type { ImportProfilesResult, Profile, ProfileListEntry } from "@/ipc/bindings";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useProfileColumnsStore } from "@/stores/column-visibility-store";
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

// `listProfiles` answers with the rows plus the number of stored profiles this
// build could not decode. Tests that only care about the rows go through these
// helpers, so the shape lives in one place instead of every mock.
function listing(entries: ProfileListEntry[], undecodableProfiles = 0) {
  return { entries, undecodableProfiles };
}

function mockProfileList(entries: ProfileListEntry[], undecodableProfiles = 0) {
  ipcMocks.listProfiles.mockResolvedValue(listing(entries, undecodableProfiles));
}

function mockProfileListOnce(entries: ProfileListEntry[], undecodableProfiles = 0) {
  ipcMocks.listProfiles.mockResolvedValueOnce(listing(entries, undecodableProfiles));
}

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
    ipcMocks.dedupeProfiles.mockResolvedValue({ kept: 0, removedProfileIds: [], total: 0 });
    ipcMocks.deleteSubscriptions.mockResolvedValue(1);
    ipcMocks.deleteProfiles.mockResolvedValue(1);
    ipcMocks.exportProfileShareLinks.mockImplementation(async (indexIds: string[]) => ({
      count: indexIds.length,
      format: "shareLinks",
      text: indexIds.map((indexId) => `vless://${indexId}@example.test:443`).join("\n"),
    }));
    ipcMocks.generateQrCode.mockResolvedValue({ mimeType: "image/svg+xml", svg: "<svg />" });
    ipcMocks.importProfilesFromText.mockResolvedValue(
      makeImportResult({ imported: 1, importedProfileIds: ["profile-new"] }),
    );
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
    mockProfileList(makeProfiles(5000));

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
      ipInfo: null,
      outcome: "timedOut",
      speedBytesPerSecond: 2048,
    };
    mockProfileList([profile]);

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    expect(screen.queryByRole("columnheader", { name: "Speed" })).not.toBeInTheDocument();
    const menu = await openRowContextMenu();
    const speedMenu = await openContextSubmenu(menu, "Speedtest");
    await userEvent.click(within(speedMenu).getByRole("menuitem", { name: "Speed" }));

    expect(await screen.findByRole("columnheader", { name: "Speed" })).toBeInTheDocument();
    expect(await screen.findByText("Request timed out")).toBeInTheDocument();
    expect(screen.queryByText("2.0 KB/s")).not.toBeInTheDocument();
  });

  it("removes multi-select and copy while keeping row actions and sorting", async () => {
    const profiles = makeProfiles(3);
    mockProfileList(profiles);

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
    mockProfileList(makeProfiles(2));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    const menu = await openRowContextMenu(1);
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Edit" }));

    const dialog = await screen.findByRole("dialog", { name: "Edit profile" });
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
    mockProfileList(makeProfiles(2));

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
    mockProfileList(makeProfiles(2));

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
    mockProfileList(makeProfiles(1));
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
    mockProfileList(makeProfiles(3));

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
    mockProfileList([]);

    renderProfiles();

    expect(await screen.findByText("No profiles")).toBeInTheDocument();
    expect(
      screen.getByText("Add a profile or import one from a subscription to get started."),
    ).toBeInTheDocument();
    expect(screen.queryByTestId("server-row")).not.toBeInTheDocument();
  });

  it("ships high-signal columns and collapses niche ones by default", async () => {
    mockProfileList(makeProfiles(3));

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
    mockProfileList(makeProfiles(3));

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

  it("keeps the statistics stream out of the table until a traffic column is visible", async () => {
    mockProfileList([makeProfile(0)]);

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    act(() => {
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
      });
    });

    // Traffic columns ship hidden, so the once-per-second statistics stream must
    // not rebuild the row model to update cells nobody can see.
    expect(screen.queryByText("4.0 KB")).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("menuitem", { name: "Columns" }));
    await userEvent.click(await screen.findByRole("menuitemcheckbox", { name: "Total up" }));
    await userEvent.keyboard("{Escape}");

    // Revealing the column subscribes to the live map and shows the live value.
    expect(await screen.findByRole("columnheader", { name: "Total up" })).toBeInTheDocument();
    expect(await screen.findByText("4.0 KB")).toBeInTheDocument();
  });

  it("restores default columns from the column menu reset action", async () => {
    mockProfileList(makeProfiles(3));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("menuitem", { name: "Columns" }));
    await userEvent.click(await screen.findByRole("menuitemcheckbox", { name: "IP info" }));
    expect(await screen.findByRole("columnheader", { name: "IP info" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("menuitem", { name: "Reset to defaults" }));
    expect(screen.queryByRole("columnheader", { name: "IP info" })).not.toBeInTheDocument();
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
        "3 stored profile(s) could not be read by this version and are hidden. They were most likely written by a newer build; updating VoyaVPN should show them again.",
      ),
    ).toBeInTheDocument();
  });

  it("says nothing about unreadable profiles when every stored profile decoded", async () => {
    mockProfileList(makeProfiles(2));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    expect(screen.queryByText(/could not be read by this version/)).not.toBeInTheDocument();
  });

  it("shows profile query errors instead of silently presenting an empty table", async () => {
    ipcMocks.listProfiles.mockRejectedValue(new Error("profile list failed"));

    renderProfiles();

    expect(await screen.findByText("profile list failed")).toBeInTheDocument();
  });

  it("keeps import and subscription management without a duplicate update-all action", async () => {
    mockProfileList([]);

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
    expect(screen.getByText("Imported 2 profile(s).")).toBeInTheDocument();
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
      text: "vless://uuid@example.test:443#Scanned",
    });
    ipcMocks.importProfilesFromText.mockResolvedValue(
      makeImportResult({ imported: 1, importedProfileIds: ["profile-scanned"], parsed: 1 }),
    );

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
    expect(screen.getByText("Imported 1 profile(s).")).toBeInTheDocument();
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
      listing(imported && !filter ? [importedProfile] : []),
    );
    ipcMocks.importProfilesFromText.mockImplementation(async () => {
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
    expect(
      screen.getByText("Imported 1 profile(s). 1 updated. 2 duplicate(s) removed."),
    ).toBeInTheDocument();
  });

  it("does not import when clipboard text is empty", async () => {
    const readText = mockClipboardReadText(" \n ");
    mockProfileList([]);

    renderProfiles();

    await userEvent.click(await screen.findByRole("menuitem", { name: "More actions" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Import from clipboard" }));

    await waitFor(() => expect(readText).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("Clipboard is empty.")).toBeInTheDocument();
    expect(ipcMocks.importProfilesFromText).not.toHaveBeenCalled();
  });

  it("does not import when clipboard text read is unavailable", async () => {
    mockClipboardUnavailable();
    mockProfileList([]);

    renderProfiles();

    await userEvent.click(await screen.findByRole("menuitem", { name: "More actions" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Import from clipboard" }));

    expect(await screen.findByText("Clipboard text read is unavailable in this WebView.")).toBeInTheDocument();
    expect(ipcMocks.importProfilesFromText).not.toHaveBeenCalled();
  });

  it("batch exports every profile even when the list is filtered", async () => {
    const profiles = makeProfiles(2);
    ipcMocks.listProfiles.mockImplementation(async (_subscriptionId: string | null, filter: string | null) =>
      listing(filter ? [profiles[1]!] : profiles),
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
    mockProfileList([
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
    mockProfileList([makeProfile(0)]);
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
    mockProfileList(makeProfiles(3));

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
    mockProfileList([]);

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

  it("keeps the editor open with its edits when the backend rejects the save", async () => {
    mockProfileList([]);
    ipcMocks.saveProfile.mockRejectedValue(new Error("profile address is already used"));

    renderProfiles();

    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    fireEvent.change(await screen.findByLabelText("Remarks"), { target: { value: "Rejected node" } });
    fireEvent.change(screen.getByLabelText("Address"), { target: { value: "node.example.test" } });
    fireEvent.change(screen.getByLabelText("UUID"), { target: { value: "uuid-rejected" } });
    fireEvent.click(screen.getByRole("button", { name: /Save/ }));

    await waitFor(() => expect(ipcMocks.saveProfile).toHaveBeenCalled());
    const dialog = await screen.findByRole("dialog", { name: "Add profile" });
    expect(await within(dialog).findByText("profile address is already used")).toBeInTheDocument();
    // The form remounts whenever the dialog toggles, so staying open is what
    // preserves the values the user already typed.
    expect(within(dialog).getByLabelText("Remarks")).toHaveValue("Rejected node");
    expect(within(dialog).getByLabelText("UUID")).toHaveValue("uuid-rejected");
  });

  it("shows the required credential error instead of silently refusing to save", async () => {
    mockProfileList([]);

    renderProfiles();

    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    fireEvent.change(await screen.findByLabelText("Remarks"), { target: { value: "Missing UUID" } });
    fireEvent.change(screen.getByLabelText("Address"), { target: { value: "node.example.test" } });
    fireEvent.click(screen.getByRole("button", { name: /Save/ }));

    expect(await screen.findByText("password or ID is required")).toBeInTheDocument();
    expect(screen.getByLabelText("UUID")).toHaveAttribute("aria-invalid", "true");
    expect(ipcMocks.saveProfile).not.toHaveBeenCalled();
  });

  it("saves the TUIC uuid and password into the fields the contract names", async () => {
    mockProfileList([]);

    renderProfiles();

    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    await selectComboboxOption("Protocol", "TUIC");
    expect(await screen.findByLabelText("Congestion control")).toBeInTheDocument();
    // `insecureConcurrency` belongs to Naive, not TUIC; rendering it here would
    // silently discard whatever the user typed.
    expect(screen.queryByLabelText("Insecure concurrency")).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Remarks"), { target: { value: "TUIC node" } });
    fireEvent.change(screen.getByLabelText("Address"), { target: { value: "tuic.example.test" } });
    fireEvent.change(screen.getByLabelText("UUID"), { target: { value: "uuid-tuic" } });
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "tuic-secret" } });
    fireEvent.change(screen.getByLabelText("Congestion control"), { target: { value: "bbr" } });
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

    fireEvent.click(screen.getByRole("button", { name: "Add" }));
    await selectComboboxOption("Protocol", "Naive");
    expect(await screen.findByLabelText("Insecure concurrency")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Remarks"), { target: { value: "Naive node" } });
    fireEvent.change(screen.getByLabelText("Address"), { target: { value: "naive.example.test" } });
    fireEvent.change(screen.getByLabelText("Username"), { target: { value: "naive-user" } });
    fireEvent.change(screen.getByLabelText("Password"), { target: { value: "naive-secret" } });
    fireEvent.change(screen.getByLabelText("Congestion control"), { target: { value: "bbr" } });
    fireEvent.change(screen.getByLabelText("Insecure concurrency"), { target: { value: "4" } });
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
        transport: { header: "http", host: "cdn.example.test", kind: "tcp", path: "/obfs" },
      }),
    ]);

    renderProfiles();

    expect(await screen.findByText("Obfuscated node")).toBeInTheDocument();
    const menu = await openRowContextMenu();
    await userEvent.click(within(menu).getByRole("menuitem", { name: "Edit" }));

    const dialog = await screen.findByRole("dialog", { name: "Edit profile" });
    expect(within(dialog).getByLabelText("Host")).toHaveValue("cdn.example.test");
    expect(within(dialog).getByLabelText("Path")).toHaveValue("/obfs");
    fireEvent.change(within(dialog).getByLabelText("Remarks"), { target: { value: "Renamed node" } });
    fireEvent.click(within(dialog).getByRole("button", { name: /Save/ }));

    await waitFor(() =>
      expect(ipcMocks.saveProfile).toHaveBeenCalledWith(
        expect.objectContaining({
          remarks: "Renamed node",
          transport: { header: "http", host: "cdn.example.test", kind: "tcp", path: "/obfs" },
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

      fireEvent.click(await screen.findByRole("button", { name: "新增" }));
      const dialog = await screen.findByRole("dialog", { name: "新增配置" });
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

      fireEvent.click(await screen.findByRole("button", { name: "新增" }));
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
      makeImportResult({ failed: 2, imported: 3, skipped: 1 }),
    );

    await withLocale("zh-Hans", async () => {
      renderProfiles();

      await userEvent.click(await screen.findByRole("menuitem", { name: "更多操作" }));
      await userEvent.click(await screen.findByRole("menuitem", { name: "从剪贴板导入" }));

      expect(
        await screen.findByText("已导入 3 个配置。 已跳过 1 个。 2 个解析失败。"),
      ).toBeInTheDocument();
    });
  });

  it("offers each speedtest probe exactly once in the row menu", async () => {
    mockProfileList(makeProfiles(1));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    const menu = await openRowContextMenu();
    const speedMenu = await openContextSubmenu(menu, "Speedtest");

    // "Fast" used to sit next to "Real" while dispatching the same `latency`
    // probe, so the menu offered one operation under two labels.
    expect(within(speedMenu).getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "TCP",
      "Real",
      "UDP",
      "Speed",
      "Mixed",
      "Stop",
    ]);
  });

  it("renders the shared export entries through the context-menu primitives", async () => {
    mockProfileList(makeProfiles(1));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    const menu = await openRowContextMenu();
    const exportMenu = await openContextSubmenu(menu, "Export");

    // Both menus map the same descriptor list, so the row menu must carry every
    // export kind the toolbar offers.
    expect(within(exportMenu).getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "Share links",
      "Share links (Base64)",
      "Voya profile bundle",
      "Client config",
      "Show QR",
      "Save share links",
      "Save client config",
    ]);
  });

  it("confirms before deduping and cancels without deleting duplicates", async () => {
    mockProfileList(makeProfiles(3));

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    await userEvent.click(await screen.findByRole("menuitem", { name: "More actions" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Dedupe" }));

    // Dedupe deletes rows across every subscription, so it is gated exactly the
    // way per-row delete is instead of firing from a single menu click.
    const confirm = await screen.findByRole("alertdialog");
    fireEvent.click(within(confirm).getByRole("button", { name: "Cancel" }));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(ipcMocks.dedupeProfiles).not.toHaveBeenCalled();
  });

  it("reports how many duplicates the confirmed dedupe removed", async () => {
    mockProfileList(makeProfiles(3));
    ipcMocks.dedupeProfiles.mockResolvedValue({
      kept: 2,
      removedProfileIds: ["profile-2"],
      total: 3,
    });

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    await userEvent.click(await screen.findByRole("menuitem", { name: "More actions" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Dedupe" }));
    fireEvent.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Remove duplicates" }),
    );

    await waitFor(() => expect(ipcMocks.dedupeProfiles).toHaveBeenCalledWith(null, null));
    expect(
      await screen.findByText("Removed 1 duplicate profile(s); kept 2 of 3."),
    ).toBeInTheDocument();
  });

  it("bulk exports the shareable profiles instead of failing on a policy group", async () => {
    mockProfileList([
      ...makeProfiles(2),
      makeProfile(2, {
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

    renderProfiles();

    expect(await screen.findByText("Server 0")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("menuitem", { name: "Bulk export" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Show QR" }));

    // The backend collects share links with `?`, so one policy group would
    // otherwise fail the export for every node in the list.
    await waitFor(() =>
      expect(ipcMocks.exportProfileShareLinks).toHaveBeenCalledWith(["profile-0", "profile-1"]),
    );
    expect(
      await screen.findByText("Skipped 1 profile(s) without a share link."),
    ).toBeInTheDocument();
  });

  it("builds a policy group with child picker and generator preview", async () => {
    const user = userEvent.setup();

    mockProfileList([]);
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

// Every field of the generated `ImportProfilesResult`; overriding only what a
// test cares about keeps the mocks from drifting away from the contract.
function makeImportResult(overrides: Partial<ImportProfilesResult> = {}): ImportProfilesResult {
  return {
    deduped: 0,
    discardedNodeOverrides: 0,
    failed: 0,
    filtered: 0,
    imported: 0,
    importedProfileIds: [],
    messages: [],
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
