import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { makeAppSettings } from "@/features/settings/app-settings.test-fixture";
import type { CoreState, RoutingRule, Routing_Serialize } from "@/ipc/bindings";
import { useRuntimeActionStore } from "@/stores/runtime-action-store";
import { useShellStore } from "@/stores/shell-store";

import { RoutingScreen } from "./routing-screen";

const ipc = vi.hoisted(() => ({
  connectionModeStatus: vi.fn(),
  deleteRoutingRules: vi.fn(),
  listProcessCandidates: vi.fn(),
  listProfiles: vi.fn(),
  listRoutings: vi.fn(),
  loadAppSettings: vi.fn(),
  moveRoutingRule: vi.fn(),
  proxySetTrafficMode: vi.fn(),
  resetRoutingRules: vi.fn(),
  saveRoutingRule: vi.fn(),
}));
vi.mock("@/ipc/commands", () => ipc);

const runtime = vi.hoisted(() => ({ state: "disconnected" as CoreState }));
vi.mock("@/ipc/runtime-event-store", () => ({
  useRuntimeEventStore: (select: (state: { coreState: { state: CoreState } }) => unknown) =>
    select({ coreState: { state: runtime.state } }),
}));

const clients = new Set<QueryClient>();

describe("RoutingScreen", () => {
  beforeEach(() => {
    Object.values(ipc).forEach((mock) => mock.mockReset());
    runtime.state = "disconnected";
    ipc.listRoutings.mockResolvedValue([activeRouting(), otherRouting()]);
    ipc.listProfiles.mockResolvedValue({ entries: [], undecodableProfiles: 0 });
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
    useShellStore.setState({ routingPerAppRequested: false });
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
    expect(await within(card).findByText("App-based rules only take effect in TUN mode.")).toBeInTheDocument();
  });

  it("adds a rule, then edits one on double click", async () => {
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

    await user.dblClick(screen.getByText("Office"));
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

    await user.click(screen.getByRole("button", { name: "Restore defaults" }));
    const reset = await screen.findByRole("alertdialog");
    expect(within(reset).getByRole("heading", { name: "Restore default rules?" })).toBeInTheDocument();
    await user.click(within(reset).getByRole("button", { name: "Restore" }));
    await waitFor(() => expect(ipc.resetRoutingRules).toHaveBeenCalledWith("route-active"));
  });

  it("offers a way back from global mode", async () => {
    const user = userEvent.setup();
    runtime.state = "connected";
    const settings = makeAppSettings();
    settings.proxy.trafficMode = "global";
    ipc.loadAppSettings.mockResolvedValue(settings);
    ipc.proxySetTrafficMode.mockResolvedValue({ mode: "rule" });
    renderScreen();

    expect(
      await screen.findByText(
        "Global mode is on: all captured traffic goes through the proxy and these rules are skipped.",
      ),
    ).toBeInTheDocument();
    const switchBack = screen.getByRole("button", { name: "Switch to smart routing" });
    await waitFor(() => expect(switchBack).toBeEnabled());
    await user.click(switchBack);

    await waitFor(() => expect(ipc.proxySetTrafficMode).toHaveBeenCalledWith("rule", expect.anything()));
  });

  it("opens the per-app dialog from its card and from a settings deep link", async () => {
    const user = userEvent.setup();
    renderScreen();
    await screen.findByText("Office");

    await user.click(screen.getByRole("button", { name: "Edit" }));
    expect(await screen.findByRole("heading", { name: "Per-app proxy" , level: 2 })).toBeInTheDocument();
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(useShellStore.getState().routingPerAppRequested).toBe(true);
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(useShellStore.getState().routingPerAppRequested).toBe(false));
  });

  it("reports a failed save inline", async () => {
    const user = userEvent.setup();
    ipc.saveRoutingRule.mockRejectedValue(new Error("save failed"));
    renderScreen();
    await screen.findByText("Office");

    await user.click(screen.getByRole("switch", { name: "Enable Office" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("save failed");
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
    expect(screen.getByRole("button", { name: "Restore defaults" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Edit" })).toBeDisabled();
  });

  it("shows a load failure instead of an empty state", async () => {
    ipc.listRoutings.mockRejectedValue(new Error("database locked"));
    renderScreen();

    expect(await screen.findByRole("alert")).toHaveTextContent("database locked");
    expect(screen.queryByText("No rule set is active.")).not.toBeInTheDocument();
  });
});

function renderScreen() {
  const client = new QueryClient({ defaultOptions: { queries: { gcTime: 0, retry: false } } });
  clients.add(client);
  return render(
    <QueryClientProvider client={client}>
      <RoutingScreen />
    </QueryClientProvider>,
  );
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
