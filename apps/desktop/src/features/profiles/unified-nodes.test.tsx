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
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createAppQueryClient } from "@/components/app-shell/query-client";
import { renderWithQuery } from "@/test/render";
import type {
  ProfileListEntry,
  Subscription,
  RuntimeStatusResponse,
} from "@/ipc/bindings";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { queryKeys } from "@/ipc/query-keys";
import { useRuntimeActionStore } from "@/stores/runtime-action-store";
import { useNodeListStore } from "@/stores/node-list-store";
import { makeProfileFixture } from "@/test/profile-fixture";
import { ProfilesScreen } from "./server-table";
import { nodeListRows, LOCAL_GROUP_KEY } from "./node-list-rows";

const mocks = vi.hoisted(() => ({
  exportProfileShareLinks: vi.fn(),
  generateQrCode: vi.fn(),
  listProfiles: vi.fn(),
  listSubscriptions: vi.fn(),
  listSubscriptionMetadata: vi.fn(),
  runSpeedtest: vi.fn(),
  setActiveProfile: vi.fn(),
  connectActiveProfile: vi.fn(),
  restartCore: vi.fn(),
}));
vi.mock("@/ipc/commands", async (original) => ({
  ...(await original<typeof import("@/ipc/commands")>()),
  ...mocks,
}));

const profiles = [
  makeProfileFixture(0, { remarks: "Tokyo", subscriptionId: "a" }),
  makeProfileFixture(1, { remarks: "Tokyo", subscriptionId: "b" }),
  makeProfileFixture(2, { remarks: "Paris" }),
  makeProfileFixture(3, { remarks: "Osaka", subscriptionId: "a" }),
];
function source(id: string, remarks: string, sort: number): Subscription {
  return { id, remarks, sort, additionalUrl: "", autoUpdateIntervalMinutes: null,
    converterTarget: null, enabled: true, filter: null, url: "https://example.test/sub", userAgent: "" };
}
const subscriptions = [source("b", "Backup", 1), source("a", "Asia", 0)];
const clients = new Set<QueryClient>();
const clipboardDescriptor = Object.getOwnPropertyDescriptor(
  navigator,
  "clipboard",
);
const writeText = vi.fn();
function core(
  state: RuntimeStatusResponse["state"] = "disconnected",
): RuntimeStatusResponse {
  return {
    state,
    mainPid: state === "connected" ? 42 : null,
    activeProfileId: state === "connected" ? "profile-2" : null,
    activeTunBackend: null,
    prePid: null,
    runningCoreType: state === "connected" ? "singBox" : null,
    connectedDurationMs: null,
  };
}
function renderScreen() {
  const client = createAppQueryClient();
  clients.add(client);
  return { ...renderWithQuery(<ProfilesScreen />, { queryClient: client }), client };
}
function changed() {
  clients.forEach(
    (client) =>
      void client.invalidateQueries({ queryKey: queryKeys.profiles }),
  );
}
function card(name: string) {
  return screen.getByRole("article", { name });
}
beforeEach(() => {
  vi.resetAllMocks();
  writeText.mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText },
  });
  useRuntimeActionStore.setState({
    pendingAction: null,
    modePending: false,
    switchingId: null,
  });
  useRuntimeEventStore.setState({
    coreState: core(),
    coreStateReceivedAt: null,
    speedtestResultsByProfileId: {},
    speedtestRunning: false,
  });
  mocks.listProfiles.mockResolvedValue({
    entries: profiles,
    undecodableProfiles: 0,
  });
  mocks.listSubscriptions.mockResolvedValue(subscriptions);
  mocks.listSubscriptionMetadata.mockResolvedValue([]);
  mocks.runSpeedtest.mockResolvedValue(null);
  mocks.exportProfileShareLinks.mockImplementation(async (ids: string[]) => ({
    text: ids.join("\n"),
    count: ids.length,
    format: "shareLinks",
  }));
  mocks.generateQrCode.mockResolvedValue({
    mimeType: "image/svg+xml",
    svg: "<svg></svg>",
  });
  mocks.setActiveProfile.mockResolvedValue(profiles[2]);
  mocks.connectActiveProfile.mockResolvedValue(core("connected"));
  mocks.restartCore.mockResolvedValue(core("connected"));
});
afterEach(() => {
  if (clipboardDescriptor)
    Object.defineProperty(navigator, "clipboard", clipboardDescriptor);
  else Reflect.deleteProperty(navigator, "clipboard");
  cleanup();
  clients.forEach((c) => c.clear());
  clients.clear();
});

describe("source-derived node groups", () => {
  it("orders subscriptions by source and puts all local nodes last with continuous boundaries", () => {
    const rows = nodeListRows(profiles, new Set(), "Local nodes", [...subscriptions, source("empty", "Empty", 2)], "Unknown");
    expect(rows.map((row) => [row.key, row.groupKey, row.last])).toEqual([
      ["subscription:a", "subscription:a", false],
      ["profile:profile-0", "subscription:a", false],
      ["profile:profile-3", "subscription:a", true],
      ["subscription:b", "subscription:b", false],
      ["profile:profile-1", "subscription:b", true],
      ["subscription:empty", "subscription:empty", true],
      ["local", "local", false],
      ["profile:profile-2", "local", true],
    ]);
    const collapsed = nodeListRows(profiles, new Set(["subscription:a", "subscription:b", LOCAL_GROUP_KEY]), "Local nodes", subscriptions, "Unknown");
    expect(collapsed).toHaveLength(3);
    expect(collapsed.every((row) => row.kind === "group" && row.last && !row.expanded)).toBe(true);
  });

  it("keeps same-name and unavailable subscriptions distinct from local nodes", () => {
    const rows = nodeListRows(profiles, new Set(), "Local nodes", [source("a", "Same", 0), source("b", "Same", 0)], "Unknown");
    expect(rows.filter((row) => row.kind === "group").map((row) => [row.key, row.name])).toEqual([
      ["subscription:a", "Same"], ["subscription:b", "Same"], ["local", "Local nodes"],
    ]);
    const unavailable = nodeListRows(profiles, new Set(), "Local nodes", [], "Unknown");
    expect(unavailable.filter((row) => row.kind === "group").map((row) => [row.key, row.name, row.members.length])).toEqual([
      ["subscription:a", "Unknown", 2], ["subscription:b", "Unknown", 1], ["local", "Local nodes", 1],
    ]);
    expect(nodeListRows([], new Set(), "Local nodes", [source("empty", "", 0)], "Unknown")[0]).toMatchObject({ name: "Unknown", members: [], last: true });
  });

  it("shows only populated local groups and retains empty subscriptions", async () => {
    mocks.listProfiles.mockResolvedValue({ entries: [], undecodableProfiles: 0 });
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    expect(screen.queryByRole("button", { name: "Local nodes" })).not.toBeInTheDocument();
    expect(screen.getAllByTestId("node-group-card")).toHaveLength(2);
    // An empty group has nothing to share; the menu says so by disabling it.
    await userEvent.click(within(card("Asia")).getByRole("menuitem", { name: "More actions for Asia" }));
    expect(screen.getByRole("menuitem", { name: "Share links" })).toHaveAttribute("aria-disabled", "true");
    await userEvent.keyboard("{Escape}");
    expect(within(card("Asia")).getByRole("button", { name: "Test group" })).toBeDisabled();
    expect(nodeListRows([], new Set(), "Local nodes", [], "Unknown")).toEqual([]);
  });

  it("works offline, preserves duplicate node identities and collapse state through refresh", async () => {
    renderScreen();
    const toggle = await screen.findByRole("button", { name: "Asia" });
    expect(screen.getAllByTestId("server-row")).toHaveLength(4);
    expect(screen.getAllByText("Tokyo")).toHaveLength(2);
    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
    await userEvent.click(toggle);
    expect(screen.getAllByTestId("server-row")).toHaveLength(2);
    await act(async () => changed());
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(toggle);
    expect(screen.getAllByTestId("server-row")).toHaveLength(4);
    expect(mocks.setActiveProfile).not.toHaveBeenCalled();
    expect(mocks.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("hides the local group when its final node is removed", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Local nodes" });
    mocks.listProfiles.mockResolvedValue({ entries: profiles.filter((p) => p.profile.subscriptionId), undecodableProfiles: 0 });
    await act(async () => changed());
    await waitFor(() => expect(screen.queryByRole("button", { name: "Local nodes" })).not.toBeInTheDocument());
    expect(screen.getAllByTestId("node-group-card")).toHaveLength(2);
  });

  it("uses source identity for renamed subscriptions and preserves their collapsed state", async () => {
    const { client } = renderScreen();
    await userEvent.click(await screen.findByRole("button", { name: "Asia" }));
    mocks.listSubscriptions.mockResolvedValue([source("a", "Renamed", 0), subscriptions[0]]);
    await act(async () => { await client.invalidateQueries({ queryKey: queryKeys.subscriptions }); });
    expect(await screen.findByRole("button", { name: "Renamed" })).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("button", { name: "Asia" })).not.toBeInTheDocument();
  });

  it("removes group editing, node copies and transfers from both node menus", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Local nodes" });
    expect(screen.queryByText("Create group")).not.toBeInTheDocument();
    expect(screen.queryByText("Edit group")).not.toBeInTheDocument();
    expect(screen.queryByText("Delete group")).not.toBeInTheDocument();
    for (const name of ["Asia", "Backup"])
      expect(within(card(name)).getByRole("menuitem", { name: `More actions for ${name}` })).toBeVisible();
    const row = screen.getAllByTestId("server-row").find((r) => within(r).queryByText("Paris"))!;
    for (const context of [false, true]) {
      if (context) fireEvent.contextMenu(row);
      else await userEvent.click(within(row).getByRole("menuitem", { name: "Actions for Paris" }));
      expect(screen.queryByRole("menuitem", { name: "Copy node" })).not.toBeInTheDocument();
      expect(screen.queryByRole("menuitem", { name: "Move to group" })).not.toBeInTheDocument();
      expect(screen.getByRole("menuitem", { name: "Edit" })).toBeVisible();
      expect(screen.getByRole("menuitem", { name: "Delete" })).toBeVisible();
      await userEvent.keyboard("{Escape}");
    }
    await userEvent.click(screen.getAllByRole("menuitem", { name: "Actions for Tokyo" })[0]!);
    for (const name of ["Edit", "Copy node", "Move to group", "Delete"])
      expect(screen.queryByRole("menuitem", { name })).not.toBeInTheDocument();
  });

  it("tests all source members while collapsed without selecting or connecting", async () => {
    renderScreen();
    await userEvent.click(await screen.findByRole("button", { name: "Asia" }));
    await userEvent.click(within(card("Asia")).getByRole("button", { name: "Test group" }));
    expect(mocks.runSpeedtest).toHaveBeenCalledWith(expect.objectContaining({ target: { scope: "profiles", profileIds: ["profile-0", "profile-3"] } }));
    expect(mocks.setActiveProfile).not.toHaveBeenCalled();
    expect(mocks.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("flattens a large local list with stable node IDs", () => {
    const many = Array.from({ length: 5000 }, (_, index) => makeProfileFixture(index));
    const rows = nodeListRows(many, new Set(), "Local nodes", [], "Unknown");
    expect(rows).toHaveLength(5001);
    expect(rows[5000]).toMatchObject({ key: "profile:profile-4999", groupKey: LOCAL_GROUP_KEY, last: true });
  });
});

describe("group panels and scoped export", () => {
  async function exportGroup(name: string, action: string) {
    await userEvent.click(
      // Sharing lives in the group's "more actions" menu.
      screen.getByRole("menuitem", { name: `More actions for ${name}` }),
    );
    await userEvent.click(screen.getByRole("menuitem", { name: action }));
  }

  it("exports fresh complete membership as share links while collapsed", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await userEvent.click(screen.getByRole("button", { name: "Asia" }));
    mocks.listProfiles.mockResolvedValue({ entries: profiles.map((p) => p.profile.id === "profile-2" ? { ...p, profile: { ...p.profile, subscriptionId: "a" } } : p), undecodableProfiles: 0 });
    await exportGroup("Asia", "Share links");
    await waitFor(() =>
      expect(mocks.exportProfileShareLinks).toHaveBeenCalledWith([
        "profile-0",
        "profile-2",
        "profile-3",
      ]),
    );
    expect(writeText).toHaveBeenCalledWith("profile-0\nprofile-2\nprofile-3");
    expect(mocks.setActiveProfile).not.toHaveBeenCalled();
    expect(mocks.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("exports only local nodes and keeps removed export formats absent", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await userEvent.click(
      screen.getByRole("menuitem", { name: "More actions for Local nodes" }),
    );
    expect(
      within(screen.getByRole("menu"))
        .getAllByRole("menuitem")
        .map((item) => item.textContent),
    ).toEqual(["Share links", "Show QR"]);
    await userEvent.click(
      screen.getByRole("menuitem", { name: "Share links" }),
    );
    await waitFor(() =>
      expect(mocks.exportProfileShareLinks).toHaveBeenCalledWith(["profile-2"]),
    );
  });

  it("shows group QR export and retains text if the QR is too large", async () => {
    mocks.generateQrCode.mockRejectedValue(
      new Error("QR content is too large"),
    );
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await exportGroup("Asia", "Show QR");
    const dialog = await screen.findByRole("dialog", { name: "Show QR" });
    expect(
      await within(dialog).findByText("QR content is too large"),
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText("Content")).toHaveValue(
      "profile-0\nprofile-3",
    );
  });

  it.each(["deleted", "load failed", "empty"])(
    "does not fall back to exporting all nodes when a group is %s",
    async (failure) => {
      renderScreen();
      await screen.findByRole("button", { name: "Asia" });
      if (failure === "load failed")
        mocks.listProfiles.mockRejectedValue(new Error("Profiles unavailable"));
      else mocks.listProfiles.mockResolvedValue({ entries: profiles.filter((p) => p.profile.subscriptionId !== "a"), undecodableProfiles: 0 });
      await exportGroup("Asia", "Share links");
      expect(await screen.findByRole("alert")).toBeInTheDocument();
      expect(mocks.exportProfileShareLinks).not.toHaveBeenCalled();
    },
  );

  it("skips unsupported share protocols in a group export", async () => {
    const http = makeProfileFixture(4, {
      subscriptionId: "a",
      protocol: {
        kind: "http",
        server: { address: "http.test", port: 8080 },
        username: "",
        password: "",
      },
    });
    mocks.listProfiles.mockResolvedValue({
      entries: [...profiles, http],
      undecodableProfiles: 0,
    });
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await exportGroup("Asia", "Share links");
    expect(await screen.findByText(/Skipped 1/)).toBeInTheDocument();
    expect(mocks.exportProfileShareLinks).toHaveBeenCalledWith([
      "profile-0",
      "profile-3",
    ]);
  });


  it("searches node names, addresses and subscriptions without changing saved collapse state", async () => {
    useNodeListStore.setState({ collapsedGroups: ["subscription:a"], hideUnreachable: false });
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    const search = screen.getByRole("textbox", { name: "Search node name, address or subscription" });
    fireEvent.change(search, { target: { value: " osaka " } });
    expect(screen.getAllByTestId("server-row")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Asia" })).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("1 of 4 nodes")).toBeInTheDocument();
    fireEvent.change(search, { target: { value: "ASIA" } });
    expect(screen.getAllByTestId("server-row")).toHaveLength(2);
    fireEvent.change(search, { target: { value: profiles[2]!.profile.protocol.server.address } });
    expect(screen.getByText("Paris")).toBeInTheDocument();
    fireEvent.change(search, { target: { value: "missing-node" } });
    expect(screen.getByText("No matching nodes")).toBeInTheDocument();
    expect(screen.queryByText("No nodes")).not.toBeInTheDocument();
    await userEvent.click(screen.getAllByRole("button", { name: "Clear search" })[0]!);
    expect(search).toHaveFocus();
    expect(screen.getByRole("button", { name: "Asia" })).toHaveAttribute("aria-expanded", "false");
    expect(useNodeListStore.getState().collapsedGroups).toEqual(["subscription:a"]);
  });

  it("sorts measured nodes by latency and hides unreachable ones on request", () => {
    const measured = (id: string, delayMs: number, outcome: NonNullable<ProfileListEntry["metrics"]["outcome"]> | null) =>
      ({
        isActive: false,
        metrics: { countryCode: null, delayMs, ipInfo: null, outcome, sort: 0 },
        profile: { id, remarks: id, subscriptionId: null },
      }) as unknown as ProfileListEntry;
    const nodes = [
      measured("slow", 300, "completed"),
      measured("untested", 0, null),
      measured("down", 0, "timedOut"),
      measured("fast", 80, "completed"),
    ];
    const ids = (view: Parameters<typeof nodeListRows>[5]) =>
      nodeListRows(nodes, new Set(), "Local nodes", [], "Unknown", view)
        .filter((row) => row.kind === "profile")
        .map((row) => row.key);

    expect(ids({ sortByLatency: true })).toEqual([
      "profile:fast",
      "profile:slow",
      "profile:untested",
      "profile:down",
    ]);
    expect(ids({ hideUnreachable: true })).toEqual(["profile:slow", "profile:untested", "profile:fast"]);
    const [group] = nodeListRows(nodes, new Set(), "Local nodes", [], "Unknown", { hideUnreachable: true });
    expect(group).toMatchObject({ kind: "group" });
    expect(group?.kind === "group" ? group.allMembers : []).toHaveLength(4);
  });
});
