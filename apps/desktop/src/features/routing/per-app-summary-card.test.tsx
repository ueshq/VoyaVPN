import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createTestQueryClient, renderWithQuery } from "@/test/render";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { RoutingRule, Routing_Serialize, TunStatus } from "@voya/contracts";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

import { PerAppSummaryCard } from "./per-app-summary-card";
import { installFakeCommands } from "@voya/features/test/backend";

const ipc = installFakeCommands({ connectionModeStatus: vi.fn() });

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
    expect(screen.queryByText("App-based rules only take effect in VPN mode.")).not.toBeInTheDocument();
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

  it("locks editing while global mode skips the rules", () => {
    renderCard(routing({ outbound: "direct", process: ["steam"] }), vi.fn(), true);

    const edit = screen.getByRole("button", { name: "Edit" });
    expect(edit).toBeDisabled();
    expect(edit.parentElement).toHaveAttribute(
      "title",
      "Rules don't apply in global mode. Switch to Rule to edit them.",
    );
  });

  it("is not offered where the tunnel cannot match apps", () => {
    useRuntimeEventStore.setState({ tun: macosTun });
    try {
      const { container } = renderCard(routing({ process: ["steam"] }), vi.fn());
      expect(container).toBeEmptyDOMElement();
    } finally {
      useRuntimeEventStore.setState({ tun: null });
    }
  });
});

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

function renderCard(value: Routing_Serialize, onEdit: () => void, locked?: boolean) {
  return renderWithQuery(<PerAppSummaryCard locked={locked} onEdit={onEdit} routing={value} />, {
    queryClient: createTestQueryClient({ gcTime: 0 }),
  });
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
