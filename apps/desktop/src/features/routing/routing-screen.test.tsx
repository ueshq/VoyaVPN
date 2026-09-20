import { cleanup, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { QueryClient } from "@tanstack/react-query";
import { createTestQueryClient, renderWithQuery } from "@/test/render";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { makeAppSettings } from "@voya/features/settings/app-settings.test-fixture";
import type { CoreState, RoutingRule, Routing_Serialize } from "@/ipc/bindings";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";

import { RoutingScreen } from "./routing-screen";

const ipc = vi.hoisted(() => ({
  connectionModeStatus: vi.fn(),
  deleteRoutingRules: vi.fn(),
  listPolicyGroups: vi.fn(),
  listProcessCandidates: vi.fn(),
  listProfileSummaries: vi.fn(),
  listRoutings: vi.fn(),
  loadAppSettings: vi.fn(),
  moveRoutingRule: vi.fn(),
  proxySetTrafficMode: vi.fn(),
  resetRoutingRules: vi.fn(),
  saveRoutingRule: vi.fn(),
}));
vi.mock("@/ipc/commands", async (importOriginal) => ({
  ...ipc,
  appErrorOfKind: (await importOriginal<typeof import("@/ipc/commands")>()).appErrorOfKind,
}));

const runtime = vi.hoisted(() => ({ state: "disconnected" as CoreState }));
vi.mock("@voya/client/runtime-event-store", () => ({
  useRuntimeEventStore: (select: (state: { coreState: { state: CoreState } }) => unknown) =>
    select({ coreState: { state: runtime.state } }),
  coreStateOf: (coreState: { state: CoreState } | null) => coreState?.state ?? "disconnected",
}));

const clients = new Set<QueryClient>();

describe("RoutingScreen", () => {
  beforeEach(() => {
    Object.values(ipc).forEach((mock) => mock.mockReset());
    runtime.state = "disconnected";
    ipc.listRoutings.mockResolvedValue([activeRouting(), otherRouting()]);
    ipc.listProfileSummaries.mockResolvedValue({ entries: [], undecodableProfiles: 0 });
    ipc.listPolicyGroups.mockResolvedValue({ entries: [] });
    ipc.connectionModeStatus.mockResolvedValue({
      mode: "systemProxy",
      processRulesEffective: false,
      vpnAvailable: true,
    });
    ipc.listProcessCandidates.mockResolvedValue([]);
    ipc.loadAppSettings.mockResolvedValue(makeAppSettings());
    ipc.saveRoutingRule.mockResolvedValue(activeRouting());
    ipc.deleteRoutingRules.mockResolvedValue(activeRouting());
    ipc.resetRoutingRules.mockResolvedValue(activeRouting());
    useRuntimeActionStore.setState({ modePending: false, pendingAction: null, switchingId: null });
  });

  afterEach(() => {
    cleanup();
    clients.forEach((client) => client.clear());
    clients.clear();
  });

  it("lists the active rules and shows the per-app rule as its own card", async () => {
    renderScreen();

    expect(await screen.findByText("Office")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Rules" })).toBeInTheDocument();
    expect(screen.getAllByRole("row")).toHaveLength(3);
    expect(screen.queryByText("Global mode is on", { exact: false })).not.toBeInTheDocument();

    const card = screen.getByRole("heading", { name: "Per-app proxy" }).closest("div")!.parentElement!;
    expect(within(card).getByText("Proxy these apps")).toBeInTheDocument();
    expect(within(card).getByText("steam")).toBeInTheDocument();
    expect(within(card).getByText("+1")).toBeInTheDocument();
    expect(await within(card).findByText("App-based rules only take effect in VPN mode.")).toBeInTheDocument();
  });

  it("adds a rule, then edits one by clicking its name", async () => {
    const user = userEvent.setup();
    renderScreen();
    await screen.findByText("Office");

    await user.click(screen.getByRole("button", { name: "Add rule" }));
    const create = await screen.findByRole("dialog");
    await user.type(within(create).getByLabelText("Name"), "Work");
    await user.type(within(create).getByLabelText("Domain"), "domain:work.test");
    await user.click(within(create).getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(ipc.saveRoutingRule).toHaveBeenCalledWith(
        "route-active",
        expect.objectContaining({ domain: ["domain:work.test"], remarks: "Work" }),
      ),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Office" }));
    const edit = await screen.findByRole("dialog");
    expect(within(edit).getByRole("heading", { name: "Edit rule" })).toBeInTheDocument();
    expect(within(edit).getByLabelText("Name")).toHaveValue("Office");
    await user.click(within(edit).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("confirms before deleting a rule or restoring the defaults", async () => {
    const user = userEvent.setup();
    renderScreen();
    await screen.findByText("Office");

    await user.click(screen.getByRole("menuitem", { name: "Actions for Office" }));
    await user.click(screen.getByRole("menuitem", { name: "Delete" }));
    const deletion = await screen.findByRole("alertdialog");
    expect(deletion).toHaveTextContent("“Office” will be permanently deleted.");
    await user.click(within(deletion).getByRole("button", { name: "Delete" }));
    await waitFor(() =>
      expect(ipc.deleteRoutingRules).toHaveBeenCalledWith("route-active", ["rule-office"]),
    );

    await user.click(screen.getByRole("menuitem", { name: "More" }));
    await user.click(screen.getByRole("menuitem", { name: "Restore defaults" }));
    const reset = await screen.findByRole("alertdialog");
    expect(within(reset).getByRole("heading", { name: "Restore default rules?" })).toBeInTheDocument();
    await user.click(within(reset).getByRole("button", { name: "Restore" }));
    await waitFor(() => expect(ipc.resetRoutingRules).toHaveBeenCalledWith("route-active"));
  });

  it("locks every rule control in global mode until switched back from the title", async () => {
    const user = userEvent.setup();
    runtime.state = "connected";
    const settings = makeAppSettings();
    settings.proxy.trafficMode = "global";
    ipc.loadAppSettings.mockResolvedValue(settings);
    ipc.proxySetTrafficMode.mockImplementation((mode) => {
      const next = makeAppSettings();
      next.proxy.trafficMode = mode;
      ipc.loadAppSettings.mockResolvedValue(next);
      return Promise.resolve({ mode });
    });
    renderScreen();

    expect(
      await screen.findByText(
        "Global mode is on: all captured traffic goes through the proxy and these rules are skipped.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Switch to rule mode" })).not.toBeInTheDocument();
    await screen.findByText("Office");
    for (const name of ["Add rule", "Edit"]) {
      const button = screen.getByRole("button", { name });
      expect(button).toBeDisabled();
      expect(button.parentElement).toHaveAttribute(
        "title",
        "Rules don't apply in global mode. Switch to Rule to edit them.",
      );
    }
    expect(screen.getByRole("switch", { name: "Enable Office" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Reorder Office" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("menuitem", { name: "Actions for Office" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Office" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(screen.getByRole("menuitem", { name: "More" }));
    const restore = screen.getByRole("menuitem", { name: "Restore defaults" });
    expect(restore).toHaveAttribute("aria-disabled", "true");
    expect(restore).toHaveAttribute("title", "Rules don't apply in global mode. Switch to Rule to edit them.");
    await user.keyboard("{Escape}");

    const rule = within(screen.getByRole("group", { name: "Traffic mode" })).getByRole("button", { name: "Rule" });
    await waitFor(() => expect(rule).toBeEnabled());
    await user.click(rule);

    await waitFor(() => expect(ipc.proxySetTrafficMode).toHaveBeenCalledWith("rule"));
    await waitFor(() => expect(screen.getByRole("button", { name: "Add rule" })).toBeEnabled());
    expect(screen.getByRole("button", { name: "Add rule" }).parentElement).not.toHaveAttribute("title");
    expect(screen.getByRole("switch", { name: "Enable Office" })).toBeEnabled();
    expect(
      screen.queryByText(
        "Global mode is on: all captured traffic goes through the proxy and these rules are skipped.",
      ),
    ).not.toBeInTheDocument();
  });

  it("opens the per-app dialog from its card and closes it again", async () => {
    const user = userEvent.setup();
    renderScreen();
    await screen.findByText("Office");

    await user.click(screen.getByRole("button", { name: "Edit" }));
    expect(await screen.findByRole("heading", { name: "Per-app proxy" , level: 2 })).toBeInTheDocument();
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("reports a failed save inline", async () => {
    const user = userEvent.setup();
    ipc.saveRoutingRule.mockRejectedValue(new Error("save failed"));
    renderScreen();
    await screen.findByText("Office");

    await user.click(screen.getByRole("switch", { name: "Enable Office" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("save failed");
  });

  it("keeps the rule editor open and says why the backend refused a rule", async () => {
    const user = userEvent.setup();
    ipc.saveRoutingRule.mockRejectedValue(new Error("rule set is locked"));
    renderScreen();
    await screen.findByText("Office");

    await user.click(screen.getByRole("button", { name: "Add rule" }));
    const editor = await screen.findByRole("dialog");
    await user.type(within(editor).getByLabelText("Name"), "Work");
    await user.type(within(editor).getByLabelText("Domain"), "domain:work.test");
    await user.click(within(editor).getByRole("button", { name: "Save" }));

    expect(await within(editor).findByText("rule set is locked")).toBeInTheDocument();
    expect(screen.getByRole("dialog")).toBe(editor);
  });

  it("confirms before pointing a rule with a missing node at the proxy", async () => {
    const user = userEvent.setup();
    ipc.listRoutings.mockResolvedValue([
      routing("route-active", true, [
        rule("rule-gone", { domain: ["gone.test"], outbound: "Gone node", remarks: "Gone" }),
      ]),
    ]);
    renderScreen();

    await user.click(await screen.findByRole("button", { name: "Use proxy instead" }));
    const confirm = await screen.findByRole("alertdialog");
    expect(
      within(confirm).getByRole("heading", { name: "Send “Gone” through the proxy?" }),
    ).toBeInTheDocument();
    expect(ipc.saveRoutingRule).not.toHaveBeenCalled();
    await user.click(within(confirm).getByRole("button", { name: "Use proxy" }));

    await waitFor(() =>
      expect(ipc.saveRoutingRule).toHaveBeenCalledWith(
        "route-active",
        expect.objectContaining({ id: "rule-gone", outbound: "proxy" }),
      ),
    );
  });

  it("shows placeholder rows while the rules load", async () => {
    ipc.listRoutings.mockReturnValue(new Promise(() => {}));
    renderScreen();

    expect(await screen.findByRole("status", { name: "Loading rules" })).toBeInTheDocument();
    expect(screen.queryByText("No rule set is active.")).not.toBeInTheDocument();
  });

  it("explains an empty rule set", async () => {
    ipc.listRoutings.mockResolvedValue([{ ...activeRouting(), rules: [] }]);
    renderScreen();

    expect(await screen.findByText("No rules")).toBeInTheDocument();
    expect(
      screen.getByText("All traffic goes through the proxy. Add a rule, or restore the default rules."),
    ).toBeInTheDocument();
    expect(screen.getByText("Off")).toBeInTheDocument();
  });

  it("explains a missing active rule set and disables editing", async () => {
    ipc.listRoutings.mockResolvedValue([otherRouting()]);
    renderScreen();

    expect(await screen.findByText("No rule set is active.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add rule" })).toBeDisabled();
    await userEvent.click(screen.getByRole("menuitem", { name: "More" }));
    expect(screen.getByRole("menuitem", { name: "Restore defaults" })).toHaveAttribute("aria-disabled", "true");
    await userEvent.keyboard("{Escape}");
    expect(screen.getByRole("button", { name: "Edit" })).toBeDisabled();
  });

  it("shows a load failure instead of an empty state", async () => {
    ipc.listRoutings.mockRejectedValue(new Error("database locked"));
    renderScreen();

    expect(await screen.findByRole("alert")).toHaveTextContent("database locked");
    expect(screen.queryByText("No rule set is active.")).not.toBeInTheDocument();
  });

  it("says saving a rule reconnects only while connected", async () => {
    runtime.state = "connected";
    const view = renderScreen();
    expect(await screen.findByText("Connected: saving a rule reconnects briefly.")).toBeInTheDocument();
    view.unmount();

    runtime.state = "disconnected";
    renderScreen();
    await screen.findByText("Office");
    expect(screen.queryByText("Connected: saving a rule reconnects briefly.")).not.toBeInTheDocument();
  });
});

function renderScreen() {
  const client = createTestQueryClient({ gcTime: 0 });
  clients.add(client);
  return renderWithQuery(<RoutingScreen />, { queryClient: client });
}

function activeRouting(): Routing_Serialize {
  return routing("route-active", true, [
    rule("rule-per-app", {
      outbound: "proxy",
      process: ["chrome.exe", "steam", "curl", "wget", "code"],
      remarks: "voya:per-app-proxy",
    }),
    rule("rule-ai", { domain: ["domain:openai.com"], remarks: "voya:ai-services" }),
    rule("rule-office", { ip: ["10.0.0.0/8"], outbound: "direct", remarks: "Office" }),
  ]);
}

function otherRouting(): Routing_Serialize {
  return routing("route-other", false, [rule("rule-x", { domain: ["x.test"] })]);
}

function routing(id: string, isActive: boolean, rules: RoutingRule[]): Routing_Serialize {
  return {
    enabled: true,
    icon: "",
    id,
    isActive,
    locked: false,
    remarks: id,
    rules,
    singboxDomainStrategy: "",
    singboxRulesetPath: "",
    sort: 0,
  };
}

function rule(id: string, overrides: Partial<RoutingRule>): RoutingRule {
  return {
    domain: null,
    enabled: true,
    id,
    inboundTags: null,
    ip: null,
    kind: null,
    network: null,
    outbound: "proxy",
    port: null,
    process: null,
    protocol: null,
    remarks: id,
    scope: "all",
    ...overrides,
  };
}
