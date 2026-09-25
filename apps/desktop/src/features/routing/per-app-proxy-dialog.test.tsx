import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { QueryClient } from "@tanstack/react-query";
import { createTestQueryClient, renderWithQuery } from "@/test/render";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ProcessCandidate, Routing_Serialize, RoutingRule } from "@voya/contracts";

import { PerAppProxyDialog } from "./per-app-proxy-dialog";
import { installFakeCommands } from "@voya/features/test/backend";

const ipcMocks = installFakeCommands({
  connectionModeStatus: vi.fn(),
  deleteRoutingRules: vi.fn(),
  listProcessCandidates: vi.fn(),
  listRoutings: vi.fn(),
  moveRoutingRule: vi.fn(),
  saveRoutingRule: vi.fn(),
});


const queryClients = new Set<QueryClient>();

function renderDialog(onOpenChange: (open: boolean) => void = vi.fn()) {
  const queryClient = createTestQueryClient({ gcTime: 0 });
  queryClients.add(queryClient);

  return renderWithQuery(<PerAppProxyDialog onOpenChange={onOpenChange} open />, { queryClient });
}

function routing(rules: RoutingRule[] = [], isActive = true): Routing_Serialize {
  return {
    id: "routing-1",
    isActive,
    remarks: "Active profile",
    rules,
    sort: 0,
  };
}

function makeRule(id: string, remarks: string, overrides: Partial<RoutingRule> = {}): RoutingRule {
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
    remarks,
    scope: "routing",
    ...overrides,
  };
}

function makePerAppRule(id: string, overrides: Partial<RoutingRule> = {}): RoutingRule {
  return makeRule(id, "voya:per-app-proxy", { process: ["chrome.exe"], ...overrides });
}

function candidate(processName: string, displayName = processName): ProcessCandidate {
  return {
    displayName,
    executablePath: null,
    processName,
    source: "runningProcess",
  };
}

afterEach(() => {
  queryClients.forEach((queryClient) => queryClient.clear());
  queryClients.clear();
});

describe("PerAppProxyDialog", () => {
  beforeEach(() => {
    Object.values(ipcMocks).forEach((mock) => mock.mockReset());
    ipcMocks.connectionModeStatus.mockResolvedValue({
      mode: "vpn",
      processRulesEffective: true,
      vpnAvailable: true,
    });
    ipcMocks.listProcessCandidates.mockResolvedValue([
      candidate("chrome.exe", "Google Chrome"),
      candidate("steam"),
    ]);
    ipcMocks.listRoutings.mockResolvedValue([routing()]);
    ipcMocks.moveRoutingRule.mockResolvedValue(routing());
    ipcMocks.saveRoutingRule.mockResolvedValue(routing());
  });

  it("lists enumerated process candidates with search", async () => {
    const user = userEvent.setup();
    renderDialog();

    await user.click(await screen.findByRole("button", { name: "Proxy these apps" }));
    expect(await screen.findByText("Google Chrome")).toBeInTheDocument();
    expect(screen.getByText("steam")).toBeInTheDocument();

    await user.type(screen.getByPlaceholderText("Search apps…"), "chrome");
    expect(screen.queryByText("steam")).not.toBeInTheDocument();
    expect(screen.getByText("Google Chrome")).toBeInTheDocument();
  });

  it("saves an include rule with picked and manually added processes", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    // The backend appends the new rule after the catch-all it was saved into.
    ipcMocks.saveRoutingRule.mockResolvedValue(
      routing([makeRule("rule-final", "Final proxy"), makePerAppRule("rule-new")]),
    );
    renderDialog(onOpenChange);

    await user.click(await screen.findByRole("button", { name: "Proxy these apps" }));
    await user.click(await screen.findByRole("checkbox", { name: /Google Chrome/ }));
    await user.type(screen.getByLabelText("Add a process name manually"), "custom-app");
    await user.keyboard("{Enter}");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(ipcMocks.saveRoutingRule).toHaveBeenCalledTimes(1));
    const [routingId, rule] = ipcMocks.saveRoutingRule.mock.calls[0];
    expect(routingId).toBe("routing-1");
    expect(rule).toMatchObject({
      outbound: "proxy",
      process: ["chrome.exe", "custom-app"],
      remarks: "voya:per-app-proxy",
      scope: "routing",
    });
    expect(ipcMocks.moveRoutingRule).toHaveBeenCalledWith("routing-1", "rule-new", "top", null);
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("leaves the managed rule alone when it is already the first rule", async () => {
    const user = userEvent.setup();
    ipcMocks.saveRoutingRule.mockResolvedValue(
      routing([makePerAppRule("rule-new"), makeRule("rule-final", "Final proxy")]),
    );
    renderDialog();

    await user.click(await screen.findByRole("button", { name: "Proxy these apps" }));
    await user.click(await screen.findByRole("checkbox", { name: /Google Chrome/ }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(ipcMocks.saveRoutingRule).toHaveBeenCalledTimes(1));
    expect(ipcMocks.moveRoutingRule).not.toHaveBeenCalled();
  });

  it("seeds from an existing managed rule and keeps its apps when switched off", async () => {
    const user = userEvent.setup();
    ipcMocks.listRoutings.mockResolvedValue([
      routing([
        {
          domain: null,
          enabled: true,
          id: "rule-9",
          inboundTags: null,
          ip: null,
          kind: null,
          network: null,
          outbound: "direct",
          port: null,
          process: ["steam"],
          protocol: null,
          remarks: "voya:per-app-proxy",
          scope: "routing",
        },
      ]),
    ]);
    renderDialog();

    const exclude = await screen.findByRole("button", { name: "Bypass these apps" });
    await waitFor(() => expect(exclude).toHaveAttribute("aria-pressed", "true"));

    await user.click(screen.getByRole("button", { name: "Off" }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    // Off keeps the chosen apps on a disabled rule for next time.
    await waitFor(() =>
      expect(ipcMocks.saveRoutingRule).toHaveBeenCalledWith(
        "routing-1",
        expect.objectContaining({ enabled: false, id: "rule-9", process: ["steam"] }),
      ),
    );
    expect(ipcMocks.deleteRoutingRules).not.toHaveBeenCalled();
  });

  it("deletes the managed rule when switched off with no app left", async () => {
    const user = userEvent.setup();
    ipcMocks.listRoutings.mockResolvedValue([
      routing([
        {
          domain: null,
          enabled: true,
          id: "rule-9",
          inboundTags: null,
          ip: null,
          kind: null,
          network: null,
          outbound: "direct",
          port: null,
          process: ["steam"],
          protocol: null,
          remarks: "voya:per-app-proxy",
          scope: "routing",
        },
      ]),
    ]);
    renderDialog();

    const exclude = await screen.findByRole("button", { name: "Bypass these apps" });
    await waitFor(() => expect(exclude).toHaveAttribute("aria-pressed", "true"));

    await user.click(screen.getByRole("button", { name: "Remove steam" }));
    await user.click(screen.getByRole("button", { name: "Off" }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(ipcMocks.deleteRoutingRules).toHaveBeenCalledWith("routing-1", ["rule-9"]),
    );
    expect(ipcMocks.saveRoutingRule).not.toHaveBeenCalled();
  });

  it("makes the TUN-only hint prominent when the mode is not TUN", async () => {
    ipcMocks.connectionModeStatus.mockResolvedValue({
      mode: "systemProxy",
      processRulesEffective: false,
      vpnAvailable: true,
    });

    renderDialog();

    expect(
      await screen.findByText("App-based rules only take effect in VPN mode."),
    ).toBeInTheDocument();
  });

  it("explains when no active routing profile exists", async () => {
    ipcMocks.listRoutings.mockResolvedValue([routing([], false)]);

    renderDialog();

    expect(
      await screen.findByText("No rule set is active, so per-app rules cannot be saved."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  });
  it("will not save a per-app rule without any app", async () => {
    const user = userEvent.setup();
    renderDialog();

    await user.click(await screen.findByRole("button", { name: "Proxy these apps" }));
    expect(screen.getByText("Choose at least one app, or turn per-app proxy off.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    expect(ipcMocks.deleteRoutingRules).not.toHaveBeenCalled();
    expect(ipcMocks.saveRoutingRule).not.toHaveBeenCalled();
  });
});
