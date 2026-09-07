import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { RoutingRule, Routing_Serialize } from "@/ipc/bindings";

import { useRoutingScreen } from "./use-routing-screen";

const ipcMocks = vi.hoisted(() => ({
  deleteRoutingRules: vi.fn(),
  deleteRoutings: vi.fn(),
  listRoutings: vi.fn(),
  moveRoutingRule: vi.fn(),
  saveRouting: vi.fn(),
  saveRoutingRule: vi.fn(),
  setActiveRouting: vi.fn(),
}));

vi.mock("@/ipc", () => ipcMocks);

const clients = new Set<QueryClient>();

describe("useRoutingScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    ipcMocks.listRoutings.mockResolvedValue([routing("route-a", true), routing("route-b", false)]);
    ipcMocks.deleteRoutingRules.mockResolvedValue(1);
    ipcMocks.deleteRoutings.mockResolvedValue(1);
    ipcMocks.moveRoutingRule.mockResolvedValue(undefined);
    ipcMocks.setActiveRouting.mockResolvedValue(undefined);
  });

  afterEach(() => {
    clients.forEach((client) => client.clear());
    clients.clear();
  });

  it("selects routings and executes activate, move, rule delete, and routing delete", async () => {
    const { result } = renderController();
    await waitFor(() => expect(result.current.selectedRouting?.id).toBe("route-a"));

    act(() => result.current.selectRouting("route-b"));
    expect(result.current.selectedRouting?.id).toBe("route-b");
    expect(result.current.selectedRule?.id).toBe("rule-route-b");

    act(() => {
      result.current.activateSelectedRouting();
      result.current.moveSelectedRule("down");
      result.current.requestDeleteRule();
    });
    await waitFor(() => expect(ipcMocks.setActiveRouting).toHaveBeenCalledWith("route-b"));
    expect(ipcMocks.moveRoutingRule).toHaveBeenCalledWith("route-b", "rule-route-b", "down", null);
    // Both deletes are unrecoverable, so nothing is sent until confirmation.
    expect(result.current.pendingDelete).toBe("rule");
    expect(ipcMocks.deleteRoutingRules).not.toHaveBeenCalled();

    act(() => result.current.confirmDelete());
    await waitFor(() => expect(ipcMocks.deleteRoutingRules).toHaveBeenCalledWith("route-b", ["rule-route-b"]));

    act(() => result.current.requestDeleteRouting());
    expect(result.current.pendingDelete).toBe("routing");
    expect(ipcMocks.deleteRoutings).not.toHaveBeenCalled();

    act(() => result.current.confirmDelete());
    await waitFor(() => expect(ipcMocks.deleteRoutings).toHaveBeenCalledWith(["route-b"]));
    expect(result.current.pendingDelete).toBeNull();
  });

  it("drops a pending delete that is dismissed instead of confirmed", async () => {
    const { result } = renderController();
    await waitFor(() => expect(result.current.selectedRouting?.id).toBe("route-a"));

    act(() => result.current.requestDeleteRouting());
    act(() => result.current.setPendingDelete(null));
    act(() => result.current.confirmDelete());

    expect(ipcMocks.deleteRoutings).not.toHaveBeenCalled();
    expect(ipcMocks.deleteRoutingRules).not.toHaveBeenCalled();
  });

  it("creates and edits routings while keeping a failed editor open", async () => {
    const { result } = renderController();
    await waitFor(() => expect(result.current.selectedRouting).not.toBeNull());
    const payload = {
      ...routing("new-route", false),
      domainStrategy: "AsIs" as const,
      singboxDomainStrategy: "" as const,
    };
    ipcMocks.saveRouting.mockResolvedValueOnce(payload);

    act(() => result.current.setRoutingDialog({ mode: "create" }));
    await act(() => result.current.handleSaveRouting(payload));
    expect(ipcMocks.saveRouting).toHaveBeenCalledWith(payload);
    expect(result.current.routingDialog).toBeNull();

    act(() => result.current.setRoutingDialog({ mode: "edit", routing: payload }));
    ipcMocks.saveRouting.mockRejectedValueOnce(new Error("routing save failed"));
    await act(() => result.current.handleSaveRouting(payload));
    expect(result.current.operationError).toBe("routing save failed");
    expect(result.current.routingDialog).toEqual({ mode: "edit", routing: payload });
  });

  it("creates a rule, selects the returned id, and keeps failed rule state recoverable", async () => {
    // The routing already owns a rule, so selecting the created one cannot be
    // confused with the fallback to the routing's first rule.
    const afterCreate = {
      ...routing("route-a", true),
      rules: [rule("rule-route-a", "route-a"), rule("created-rule", "Created")],
    };
    ipcMocks.listRoutings
      .mockResolvedValueOnce([routing("route-a", true), routing("route-b", false)])
      .mockResolvedValue([afterCreate, routing("route-b", false)]);
    const { result } = renderController();
    await waitFor(() => expect(result.current.selectedRouting?.id).toBe("route-a"));
    const payload = rule("", "Created");
    ipcMocks.saveRoutingRule.mockResolvedValueOnce(afterCreate);

    act(() => result.current.setRuleDialog({ mode: "create" }));
    await act(() => result.current.handleSaveRule(payload));
    expect(ipcMocks.saveRoutingRule).toHaveBeenCalledWith("route-a", payload);
    await waitFor(() => expect(result.current.selectedRule?.id).toBe("created-rule"));
    expect(result.current.ruleDialog).toBeNull();

    act(() => result.current.setRuleDialog({ mode: "edit", rule: payload }));
    ipcMocks.saveRoutingRule.mockRejectedValueOnce(new Error("rule save failed"));
    await act(() => result.current.handleSaveRule(payload));
    expect(result.current.operationError).toBe("rule save failed");
    expect(result.current.ruleDialog?.mode).toBe("edit");
  });

  it("keeps the edited rule selected when saving an existing rule", async () => {
    const existing = {
      ...routing("route-a", true),
      rules: [rule("rule-route-a", "route-a"), rule("rule-second", "Second")],
    };
    ipcMocks.listRoutings.mockResolvedValue([existing, routing("route-b", false)]);
    ipcMocks.saveRoutingRule.mockResolvedValue(existing);
    const { result } = renderController();
    await waitFor(() => expect(result.current.selectedRouting?.id).toBe("route-a"));

    const payload = rule("rule-second", "Second");
    act(() => result.current.setRuleDialog({ mode: "edit", rule: payload }));
    await act(() => result.current.handleSaveRule(payload));

    await waitFor(() => expect(result.current.selectedRule?.id).toBe("rule-second"));
  });

  it("does not mutate when no routing or rule is selected", async () => {
    ipcMocks.listRoutings.mockResolvedValue([]);
    const { result } = renderController();
    await waitFor(() => expect(result.current.routings).toEqual([]));

    await act(() => result.current.handleSaveRule(rule("new", "New")));
    act(() => {
      result.current.activateSelectedRouting();
      result.current.requestDeleteRouting();
      result.current.moveSelectedRule("up");
      result.current.requestDeleteRule();
    });
    act(() => result.current.confirmDelete());

    expect(ipcMocks.saveRoutingRule).not.toHaveBeenCalled();
    expect(ipcMocks.setActiveRouting).not.toHaveBeenCalled();
    expect(ipcMocks.deleteRoutings).not.toHaveBeenCalled();
    expect(ipcMocks.moveRoutingRule).not.toHaveBeenCalled();
    expect(ipcMocks.deleteRoutingRules).not.toHaveBeenCalled();
  });
});

function renderController() {
  const client = new QueryClient({ defaultOptions: { queries: { gcTime: 0, retry: false } } });
  clients.add(client);
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return renderHook(() => useRoutingScreen(), { wrapper });
}

function routing(id: string, isActive: boolean): Routing_Serialize {
  return {
    domainStrategy: "AsIs",
    enabled: true,
    icon: "",
    id,
    isActive,
    locked: false,
    remarks: id,
    rules: [rule(`rule-${id}`, id)],
    singboxDomainStrategy: "",
    singboxRulesetPath: "",
    sort: 0,
    sourceUrl: "",
  };
}

function rule(id: string, remarks: string): RoutingRule {
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
  };
}
