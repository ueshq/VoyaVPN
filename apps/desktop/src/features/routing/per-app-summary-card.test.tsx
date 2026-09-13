import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { RoutingRule, Routing_Serialize } from "@/ipc/bindings";

import { PerAppSummaryCard } from "./per-app-summary-card";

const ipc = vi.hoisted(() => ({ connectionModeStatus: vi.fn() }));
vi.mock("@/ipc/commands", () => ipc);

describe("PerAppSummaryCard", () => {
  beforeEach(() => {
    ipc.connectionModeStatus.mockReset().mockResolvedValue({
      mode: "vpn",
      processRulesEffective: true,
      vpnAvailable: true,
    });
  });

  it("lists the bypassed apps without a TUN warning while TUN is on", async () => {
    const user = userEvent.setup();
    const onEdit = vi.fn();
    renderCard(routing({ outbound: "direct", process: ["steam"] }), onEdit);

    expect(screen.getByText("Bypass these apps")).toBeInTheDocument();
    expect(screen.getByText("steam")).toBeInTheDocument();
    expect(screen.getByText("Applied before every rule below.")).toBeInTheDocument();
    await screen.findByText("steam");
    expect(screen.queryByText("App-based rules only take effect in TUN mode.")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Edit" }));
    expect(onEdit).toHaveBeenCalledOnce();
  });

  it("reads a switched-off rule as off", () => {
    renderCard(routing({ enabled: false, process: ["steam"] }), vi.fn());

    expect(screen.getByText("Off")).toBeInTheDocument();
    expect(screen.queryByText("steam")).not.toBeInTheDocument();
    expect(
      screen.getByText("Route specific applications through or around the proxy."),
    ).toBeInTheDocument();
  });
});

function renderCard(value: Routing_Serialize, onEdit: () => void) {
  const client = new QueryClient({ defaultOptions: { queries: { gcTime: 0, retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <PerAppSummaryCard onEdit={onEdit} routing={value} />
    </QueryClientProvider>,
  );
}

function routing(overrides: Partial<RoutingRule>): Routing_Serialize {
  return {
    enabled: true,
    icon: "",
    id: "route",
    isActive: true,
    locked: false,
    remarks: "Route",
    rules: [
      {
        domain: null,
        enabled: true,
        id: "rule-per-app",
        inboundTags: null,
        ip: null,
        kind: null,
        network: null,
        outbound: "proxy",
        port: null,
        process: null,
        protocol: null,
        remarks: "voya:per-app-proxy",
        scope: "routing",
        ...overrides,
      },
    ],
    singboxDomainStrategy: "",
    singboxRulesetPath: "",
    sort: 0,
  };
}
