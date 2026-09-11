import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClientProvider, type QueryClient } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createAppQueryClient } from "@/components/app-shell/query-client";
import type {
  NodeGroupAssignment,
  NodeGroupsSnapshot,
  RuntimeStatusResponse,
} from "@/ipc/bindings";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { queryKeys } from "@/ipc/query-keys";
import { useRuntimeActionStore } from "@/stores/runtime-action-store";
import { makeProfileFixture } from "@/test/profile-fixture";
import { ProfilesScreen } from "./server-table";
import { nodeListRows, UNASSIGNED_GROUP_KEY } from "./node-list-rows";

const mocks = vi.hoisted(() => ({
  copyProfiles: vi.fn(),
  listNodeGroups: vi.fn(),
  saveNodeGroup: vi.fn(),
  updateNodeGroup: vi.fn(),
  exportProfileShareLinks: vi.fn(),
  exportProfileShareLinksBase64: vi.fn(),
  exportProfileVoyaBundle: vi.fn(),
  saveTextFile: vi.fn(),
  generateQrCode: vi.fn(),
  deleteNodeGroup: vi.fn(),
  moveNodeGroup: vi.fn(),
  assignNodeGroups: vi.fn(),
  listProfiles: vi.fn(),
  listSubscriptions: vi.fn(),
  runSpeedtest: vi.fn(),
  setActiveProfile: vi.fn(),
  connectActiveProfile: vi.fn(),
  restartCore: vi.fn(),
}));
vi.mock("@/ipc/commands", async (original) => ({
  ...(await original<typeof import("@/ipc/commands")>()),
  ...mocks,
}));
vi.mock("@/ipc/file-dialog", () => ({ saveTextFile: mocks.saveTextFile }));
let groups: NodeGroupsSnapshot;
const profiles = [
  makeProfileFixture(0, { remarks: "Tokyo" }),
  makeProfileFixture(1, { remarks: "Tokyo" }),
  makeProfileFixture(2, { remarks: "Paris" }),
  makeProfileFixture(3, { remarks: "Osaka" }),
];
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
  return {
    ...render(
      <QueryClientProvider client={client}>
        <ProfilesScreen />
      </QueryClientProvider>,
    ),
    client,
  };
}
function changed() {
  clients.forEach(
    (client) =>
      void client.invalidateQueries({ queryKey: queryKeys.nodeGroups }),
  );
}
function card(name: string) {
  return screen.getByRole("article", { name });
}
async function menu(name: string, action: string) {
  if (action === "Edit group") {
    await userEvent.click(
      within(card(name)).getByRole("button", { name: action }),
    );
    return;
  }
  await userEvent.click(
    within(card(name)).getByRole("menuitem", { name: `Actions for ${name}` }),
  );
  await userEvent.click(await screen.findByRole("menuitem", { name: action }));
}
beforeEach(() => {
  vi.resetAllMocks();
  writeText.mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText },
  });
  groups = {
    groups: [
      { id: "a", name: "Asia", sort: 0 },
      { id: "b", name: "Backup", sort: 1 },
    ],
    memberships: [
      { profileId: "profile-0", groupId: "a" },
      { profileId: "profile-3", groupId: "a" },
      { profileId: "profile-1", groupId: "b" },
    ],
  };
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
  mocks.listNodeGroups.mockImplementation(async () => structuredClone(groups));
  mocks.listProfiles.mockResolvedValue({
    entries: profiles,
    undecodableProfiles: 0,
  });
  mocks.listSubscriptions.mockResolvedValue([]);
  mocks.runSpeedtest.mockResolvedValue(null);
  mocks.saveNodeGroup.mockImplementation(async (id, name) => {
    const group = { id: id ?? "new", name: name.trim(), sort: 2 };
    groups.groups = [...groups.groups.filter((g) => g.id !== id), group];
    changed();
    return group;
  });
  mocks.deleteNodeGroup.mockImplementation(async (id) => {
    groups.groups = groups.groups.filter((g) => g.id !== id);
    groups.memberships = groups.memberships.filter((m) => m.groupId !== id);
    changed();
    return null;
  });
  mocks.assignNodeGroups.mockImplementation(
    async (changes: NodeGroupAssignment[]) => {
      for (const c of changes) {
        groups.memberships = groups.memberships.filter(
          (m) => m.profileId !== c.profileId,
        );
        if (c.groupId)
          groups.memberships.push({
            profileId: c.profileId,
            groupId: c.groupId,
          });
      }
      changed();
      return null;
    },
  );
  mocks.updateNodeGroup.mockImplementation(
    async (id, name, changes: NodeGroupAssignment[]) => {
      const group = groups.groups.find((g) => g.id === id)!;
      group.name = name.trim();
      for (const c of changes) {
        groups.memberships = groups.memberships.filter(
          (m) => m.profileId !== c.profileId,
        );
        if (c.groupId)
          groups.memberships.push({
            profileId: c.profileId,
            groupId: c.groupId,
          });
      }
      changed();
      return group;
    },
  );
  for (const [exporter, format] of [
    [mocks.exportProfileShareLinks, "shareLinks"],
    [mocks.exportProfileShareLinksBase64, "shareLinksBase64"],
    [mocks.exportProfileVoyaBundle, "voyaBundle"],
  ] as const)
    exporter.mockImplementation(async (ids: string[]) => ({
      text: ids.join("\n"),
      count: ids.length,
      format,
    }));
  mocks.saveTextFile.mockResolvedValue("/tmp/nodes.txt");
  mocks.generateQrCode.mockResolvedValue({
    mimeType: "image/svg+xml",
    svg: "<svg></svg>",
  });
  mocks.copyProfiles.mockResolvedValue([]);
  mocks.moveNodeGroup.mockResolvedValue(null);
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

describe("manual node folders", () => {
  it("works offline, opens multiple groups and keeps duplicate names as separate nodes", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    expect(screen.getAllByTestId("server-row")).toHaveLength(4);
    await userEvent.click(screen.getByRole("button", { name: "Asia" }));
    await userEvent.click(screen.getByRole("button", { name: "Backup" }));
    expect(screen.getAllByTestId("server-row")).toHaveLength(1);
    await userEvent.click(screen.getByRole("button", { name: "Asia" }));
    await userEvent.click(screen.getByRole("button", { name: "Backup" }));
    expect(screen.getAllByTestId("server-row")).toHaveLength(4);
    expect(screen.getAllByText("Tokyo")).toHaveLength(2);
    expect(mocks.connectActiveProfile).not.toHaveBeenCalled();
    expect(mocks.setActiveProfile).not.toHaveBeenCalled();
  });
  it("has no page search and preserves collapsed groups through query refresh", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Asia" }));
    await act(async () => changed());
    expect(screen.getByRole("button", { name: "Asia" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    expect(screen.getByRole("button", { name: "Backup" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
  });
  it("creates an expanded group and rejects duplicate names", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await userEvent.click(screen.getByRole("menuitem", { name: /Add/ }));
    await userEvent.click(
      screen.getByRole("menuitem", { name: "Create group" }),
    );
    const input = screen.getByRole("textbox", { name: "Group name" });
    await userEvent.type(input, "Asia");
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    await userEvent.clear(input);
    await userEvent.type(input, "Work");
    await userEvent.keyboard("{Enter}");
    expect(await screen.findByRole("button", { name: "Work" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(mocks.saveNodeGroup).toHaveBeenCalledWith(null, "Work");
  });
  it("renames a group without connecting", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await menu("Asia", "Edit group");
    const input = screen.getByRole("textbox", { name: "Group name" });
    await userEvent.clear(input);
    await userEvent.type(input, "Travel");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(
      await screen.findByRole("button", { name: "Travel" }),
    ).toBeInTheDocument();
    expect(mocks.connectActiveProfile).not.toHaveBeenCalled();
  });
  it("deletes a folder while retaining its nodes", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await menu("Asia", "Delete group");
    await userEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Delete group",
      }),
    );
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "Asia" }),
      ).not.toBeInTheDocument(),
    );
    expect(screen.getAllByTestId("server-row")).toHaveLength(4);
  });
  it("saves the name and one membership delta together", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await menu("Asia", "Edit group");
    const dialog = screen.getByRole("dialog");
    await userEvent.click(
      within(dialog).getByRole("checkbox", { name: "Osaka" }),
    );
    await userEvent.click(
      within(dialog).getByRole("checkbox", { name: "Paris" }),
    );
    fireEvent.change(
      within(dialog).getByRole("textbox", { name: "Group name" }),
      { target: { value: "Travel" } },
    );
    await userEvent.click(within(dialog).getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(mocks.updateNodeGroup).toHaveBeenCalledWith("a", "Travel", [
        { profileId: "profile-3", groupId: null },
        { profileId: "profile-2", groupId: "a" },
      ]),
    );
    expect(mocks.setActiveProfile).not.toHaveBeenCalled();
  });
  it("keeps member edits and reports a failed save", async () => {
    mocks.updateNodeGroup.mockRejectedValue(
      new Error("Membership save failed"),
    );
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await menu("Asia", "Edit group");
    await userEvent.click(screen.getByRole("checkbox", { name: "Paris" }));
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Membership save failed",
    );
    expect(screen.getByRole("checkbox", { name: "Paris" })).toBeChecked();
  });
  it("tests all members even when the group is collapsed", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await userEvent.click(screen.getByRole("button", { name: "Asia" }));
    await userEvent.click(
      within(card("Asia")).getByRole("button", { name: "Test group" }),
    );
    expect(mocks.runSpeedtest).toHaveBeenCalledWith({
      target: { scope: "profiles", profileIds: ["profile-0", "profile-3"] },
    });
  });
  it("keeps node management usable when group loading fails", async () => {
    mocks.listNodeGroups.mockRejectedValue(new Error("Groups unavailable"));
    renderScreen();
    expect(await screen.findByText("Groups unavailable")).toBeInTheDocument();
    expect(screen.getAllByTestId("server-row")).toHaveLength(4);
  });
  it.each(["disconnected", "connected"] as const)(
    "uses the ordinary activation flow while %s",
    async (state) => {
      useRuntimeEventStore.setState({ coreState: core(state) });
      renderScreen();
      await screen.findByRole("button", { name: "Asia" });
      if (state === "connected")
        await act(async () =>
          useRuntimeEventStore.setState({
            coreState: { ...core(state), activeProfileId: "profile-0" },
          }),
        );
      await userEvent.click(
        within(screen.getByText("Paris").closest("article")!).getByRole(
          "button",
          { name: "Use node" },
        ),
      );
      expect(mocks.setActiveProfile).toHaveBeenCalledWith("profile-2");
      await waitFor(() =>
        expect(
          state === "connected"
            ? mocks.restartCore
            : mocks.connectActiveProfile,
        ).toHaveBeenCalledOnce(),
      );
    },
  );
  it("flattens large lists with stable node identities", () => {
    const many = Array.from({ length: 5000 }, (_, i) => makeProfileFixture(i));
    const snapshot = {
      groups: groups.groups,
      memberships: many.map((p) => ({ profileId: p.profile.id, groupId: "a" })),
    };
    const rows = nodeListRows(many, snapshot, new Set(), "Unassigned");
    expect(rows).toHaveLength(5002);
    expect(new Set(rows.map((r) => r.key)).size).toBe(rows.length);
  });
  it("reorders folders without choosing a node", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await menu("Asia", "Move down");
    expect(mocks.moveNodeGroup).toHaveBeenCalledWith("a", "down");
    await menu("Backup", "Move up");
    expect(mocks.moveNodeGroup).toHaveBeenCalledWith("b", "up");
    expect(mocks.setActiveProfile).not.toHaveBeenCalled();
  });
  it("moves a saved node to a group from its menu", async () => {
    renderScreen();
    await screen.findByText("Paris");
    await userEvent.click(
      screen.getByRole("menuitem", { name: "Actions for Paris" }),
    );
    const submenu = screen.getByRole("menuitem", { name: "Move to group" });
    submenu.focus();
    await userEvent.keyboard("{ArrowRight}");
    await userEvent.click(
      await screen.findByRole("menuitem", { name: "Asia" }),
    );
    expect(mocks.assignNodeGroups).toHaveBeenCalledWith([
      { profileId: "profile-2", groupId: "a" },
    ]);
    expect(mocks.setActiveProfile).not.toHaveBeenCalled();
  });
  it("copies a node through the saved-node command", async () => {
    renderScreen();
    await screen.findByText("Paris");
    await userEvent.click(
      screen.getByRole("menuitem", { name: "Actions for Paris" }),
    );
    await userEvent.click(screen.getByRole("menuitem", { name: "Copy node" }));
    expect(mocks.copyProfiles).toHaveBeenCalledWith(["profile-2"]);
    expect(mocks.setActiveProfile).not.toHaveBeenCalled();
  });
  it("selects search results in the member dialog and cancels without writing", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await menu("Asia", "Edit group");
    const dialog = within(screen.getByRole("dialog"));
    fireEvent.change(dialog.getByRole("searchbox"), {
      target: { value: "Tokyo" },
    });
    expect(dialog.getAllByRole("checkbox")).toHaveLength(2);
    await userEvent.click(
      dialog.getByRole("button", { name: "Select results" }),
    );
    expect(
      dialog
        .getAllByRole("checkbox")
        .every((box) => box.getAttribute("aria-checked") === "true"),
    ).toBe(true);
    await userEvent.click(
      dialog.getByRole("button", { name: "Clear results" }),
    );
    expect(
      dialog
        .getAllByRole("checkbox")
        .every((box) => box.getAttribute("aria-checked") === "false"),
    ).toBe(true);
    fireEvent.change(dialog.getByRole("searchbox"), {
      target: { value: "no-such-member" },
    });
    expect(dialog.getByText("No matching nodes.")).toBeInTheDocument();
    await userEvent.click(dialog.getByRole("button", { name: "Cancel" }));
    expect(mocks.updateNodeGroup).not.toHaveBeenCalled();
    await waitFor(() =>
      expect(
        within(card("Asia")).getByRole("button", { name: "Edit group" }),
      ).toHaveFocus(),
    );
  });
  it("keeps a failed rename editable and blocks duplicate submissions", async () => {
    let reject!: (error: Error) => void;
    mocks.updateNodeGroup.mockImplementation(
      () =>
        new Promise((_, rejectSave) => {
          reject = rejectSave;
        }),
    );
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await menu("Asia", "Edit group");
    const name = screen.getByRole("textbox", { name: "Group name" });
    await userEvent.clear(name);
    await userEvent.type(name, "Travel");
    await userEvent.dblClick(screen.getByRole("button", { name: "Save" }));
    expect(mocks.updateNodeGroup).toHaveBeenCalledOnce();
    await userEvent.keyboard("{Escape}");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    await act(async () => reject(new Error("Rename unavailable")));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Rename unavailable",
    );
    expect(name).toHaveValue("Travel");
  });
});

describe("group panels and scoped export", () => {
  async function exportGroup(name: string, action: string) {
    await userEvent.click(
      screen.getByRole("menuitem", { name: `Export ${name}` }),
    );
    await userEvent.click(screen.getByRole("menuitem", { name: action }));
  }

  it("marks continuous panel boundaries, including empty and unassigned groups", () => {
    const empty = { id: "empty", name: "Empty", sort: 2 };
    const rows = nodeListRows(
      profiles,
      { ...groups, groups: [...groups.groups, empty] },
      new Set(),
      "Unassigned",
    );
    expect(rows.map((row) => [row.key, row.groupKey, row.last])).toEqual([
      ["manual:a", "manual:a", false],
      ["profile:profile-0", "manual:a", false],
      ["profile:profile-3", "manual:a", true],
      ["manual:b", "manual:b", false],
      ["profile:profile-1", "manual:b", true],
      ["manual:empty", "manual:empty", true],
      ["unassigned", UNASSIGNED_GROUP_KEY, false],
      ["profile:profile-2", UNASSIGNED_GROUP_KEY, true],
    ]);
    const collapsed = nodeListRows(
      profiles,
      groups,
      new Set(["manual:a", "manual:b", UNASSIGNED_GROUP_KEY]),
      "Unassigned",
    );
    expect(collapsed).toHaveLength(3);
    expect(
      collapsed.every(
        (row) => row.kind === "group" && row.last && !row.expanded,
      ),
    ).toBe(true);
  });

  it("shows an empty panel and disables its export", async () => {
    groups.memberships = groups.memberships.filter((m) => m.groupId !== "b");
    renderScreen();
    await screen.findByRole("button", { name: "Backup" });
    expect(
      within(card("Backup")).getByText(/No nodes in this group/),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("menuitem", { name: "Export Backup" }),
    ).toBeDisabled();
    expect(
      within(card("Unassigned")).queryByRole("button", { name: "Edit group" }),
    ).not.toBeInTheDocument();
  });

  it.each([
    ["Share links", "exportProfileShareLinks"],
    ["Share links (Base64)", "exportProfileShareLinksBase64"],
    ["Voya node bundle", "exportProfileVoyaBundle"],
  ] as const)(
    "exports fresh complete membership as %s while collapsed",
    async (label, command) => {
      renderScreen();
      await screen.findByRole("button", { name: "Asia" });
      await userEvent.click(screen.getByRole("button", { name: "Asia" }));
      groups.memberships.push({ profileId: "profile-2", groupId: "a" });
      await exportGroup("Asia", label);
      await waitFor(() =>
        expect(mocks[command]).toHaveBeenCalledWith([
          "profile-0",
          "profile-2",
          "profile-3",
        ]),
      );
      expect(writeText).toHaveBeenCalledWith("profile-0\nprofile-2\nprofile-3");
      expect(mocks.setActiveProfile).not.toHaveBeenCalled();
      expect(mocks.connectActiveProfile).not.toHaveBeenCalled();
    },
  );

  it("exports only unassigned nodes and keeps removed export formats absent", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await userEvent.click(
      screen.getByRole("menuitem", { name: "Export Unassigned" }),
    );
    expect(
      screen
        .getAllByRole("menuitem")
        .map((item) => item.textContent)
        .filter(
          (text) =>
            text?.includes("Client config") ||
            text?.includes("Save client config"),
        ),
    ).toEqual([]);
    await userEvent.click(
      screen.getByRole("menuitem", { name: "Share links" }),
    );
    await waitFor(() =>
      expect(mocks.exportProfileShareLinks).toHaveBeenCalledWith(["profile-2"]),
    );
  });

  it("saves group share links through the existing file dialog", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await exportGroup("Asia", "Save share links");
    await waitFor(() =>
      expect(mocks.saveTextFile).toHaveBeenCalledWith({
        defaultPath: "voyavpn-share-links.txt",
        filters: [{ extensions: ["txt"], name: "Text" }],
        text: "profile-0\nprofile-3",
      }),
    );
    expect(writeText).not.toHaveBeenCalled();
    expect(await screen.findByText(/\/tmp\/nodes.txt/)).toBeInTheDocument();
  });

  it("does not report success when saving is cancelled", async () => {
    mocks.saveTextFile.mockResolvedValue(null);
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await exportGroup("Asia", "Save share links");
    await waitFor(() => expect(mocks.saveTextFile).toHaveBeenCalledOnce());
    expect(screen.queryByText(/Saved to/)).not.toBeInTheDocument();
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
      if (failure === "deleted")
        groups.groups = groups.groups.filter((group) => group.id !== "a");
      else if (failure === "empty")
        groups.memberships = groups.memberships.filter(
          (m) => m.groupId !== "a",
        );
      else
        mocks.listNodeGroups.mockRejectedValue(new Error("Groups unavailable"));
      await exportGroup("Asia", "Share links");
      expect(await screen.findByRole("alert")).toBeInTheDocument();
      expect(mocks.exportProfileShareLinks).not.toHaveBeenCalled();
    },
  );

  it("skips unsupported share protocols but includes them in a Voya bundle", async () => {
    const http = makeProfileFixture(4, {
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
    groups.memberships.push({ profileId: http.profile.id, groupId: "a" });
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await exportGroup("Asia", "Share links");
    expect(await screen.findByText(/Skipped 1/)).toBeInTheDocument();
    expect(mocks.exportProfileShareLinks).toHaveBeenCalledWith([
      "profile-0",
      "profile-3",
    ]);
    await exportGroup("Asia", "Voya node bundle");
    await waitFor(() =>
      expect(mocks.exportProfileVoyaBundle).toHaveBeenCalledWith([
        "profile-0",
        "profile-3",
        "profile-4",
      ]),
    );
  });

  it("validates the combined name and cancels unchanged edits without writing", async () => {
    renderScreen();
    await screen.findByRole("button", { name: "Asia" });
    await menu("Asia", "Edit group");
    const name = screen.getByRole("textbox", { name: "Group name" });
    fireEvent.change(name, { target: { value: " Backup " } });
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    fireEvent.change(name, { target: { value: " " } });
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    fireEvent.change(name, { target: { value: "Asia" } });
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(mocks.updateNodeGroup).not.toHaveBeenCalled();
  });
});
