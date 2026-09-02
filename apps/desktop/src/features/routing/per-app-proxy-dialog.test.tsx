import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ProcessCandidate, Routing_Serialize, RoutingRule } from "@/ipc/bindings";

import { PerAppProxyDialog } from "./per-app-proxy-dialog";

const ipcMocks = vi.hoisted(() => ({
  connectionModeStatus: vi.fn(),
  deleteRoutingRules: vi.fn(),
  listProcessCandidates: vi.fn(),
  listRoutings: vi.fn(),
  saveRoutingRule: vi.fn(),
}));

vi.mock("@/ipc", () => ipcMocks);

const queryClients = new Set<QueryClient>();

function renderDialog(onOpenChange: (open: boolean) => void = vi.fn()) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { gcTime: 0, retry: false } },
  });
  queryClients.add(queryClient);

  return render(
    <QueryClientProvider client={queryClient}>
      <PerAppProxyDialog onOpenChange={onOpenChange} open />
    </QueryClientProvider>,
  );
}

function routing(rules: RoutingRule[] = [], isActive = true): Routing_Serialize {
  return {
    domainStrategy: "",
    enabled: true,
    icon: "",
    id: "routing-1",
    isActive,
    locked: false,
    remarks: "Active profile",
    rules,
    singboxDomainStrategy: "",
    singboxRulesetPath: "",
    sort: 0,
    sourceUrl: "",
  };
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
      pacAvailable: false,
      pacEnabled: false,
      processRulesEffective: true,
      vpnAvailable: true,
    });
    ipcMocks.listProcessCandidates.mockResolvedValue([
      candidate("chrome.exe", "Google Chrome"),
      candidate("steam"),
    ]);
    ipcMocks.listRoutings.mockResolvedValue([routing()]);
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
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("seeds from an existing managed rule and deletes it when switched off", async () => {
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

    await waitFor(() =>
      expect(ipcMocks.deleteRoutingRules).toHaveBeenCalledWith("routing-1", ["rule-9"]),
    );
    expect(ipcMocks.saveRoutingRule).not.toHaveBeenCalled();
  });

  it("makes the TUN-only hint prominent when the mode is not VPN", async () => {
    ipcMocks.connectionModeStatus.mockResolvedValue({
      mode: "systemProxy",
      pacAvailable: true,
      pacEnabled: false,
      processRulesEffective: false,
      vpnAvailable: true,
    });

    renderDialog();

    expect(
      await screen.findByText("App-based rules only take effect in VPN (TUN) mode."),
    ).toBeInTheDocument();
  });

  it("explains when no active routing profile exists", async () => {
    ipcMocks.listRoutings.mockResolvedValue([routing([], false)]);

    renderDialog();

    expect(
      await screen.findByText("No active routing profile; create and activate one first."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  });
});
