import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ProfileListEntry, RoutingRule, Routing_Serialize } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { useShellStore } from "@/stores/shell-store";

import { useRoutingScreen } from "./use-routing-screen";

const ipcMocks = vi.hoisted(() => ({
  deleteRoutingRules: vi.fn(),
  listPolicyGroups: vi.fn(),
  listProfiles: vi.fn(),
  listRoutings: vi.fn(),
  moveRoutingRule: vi.fn(),
  resetRoutingRules: vi.fn(),
  saveRoutingRule: vi.fn(),
}));

vi.mock("@/ipc/commands", () => ipcMocks);

const clients = new Set<QueryClient>();

describe("useRoutingScreen", () => {
  beforeEach(() => {
    Object.values(ipcMocks).forEach((mock) => mock.mockReset());
    ipcMocks.listRoutings.mockResolvedValue([active(), inactive()]);
    ipcMocks.listProfiles.mockResolvedValue({
      entries: [entry("Tokyo"), entry("Tokyo"), entry("Osaka")],
      undecodableProfiles: 0,
    });
    ipcMocks.listPolicyGroups.mockResolvedValue({ entries: [] });
    useShellStore.setState({ routingPerAppRequested: false });
  });

  it("offers policy groups as outbounds and points a broken outbound back at the proxy", async () => {
    ipcMocks.listPolicyGroups.mockResolvedValue({
      entries: [{ group: { id: "work", name: "Work" }, isActive: false, members: [] }],
    });
    ipcMocks.saveRoutingRule.mockResolvedValue(active());
    const { result } = renderController();

    await waitFor(() =>
      expect(result.current.groupOutbounds).toEqual([{ id: "work", name: "Work" }]),
    );
    const target = result.current.rules[0]!;
    act(() => result.current.requestFixOutbound(target));
    // Retargeting a rule waits for the user to confirm it.
    expect(result.current.pendingConfirm).toEqual({ kind: "fixOutbound", rule: target });
    expect(ipcMocks.saveRoutingRule).not.toHaveBeenCalled();
    act(() => result.current.confirmPending());
    await waitFor(() =>
      expect(ipcMocks.saveRoutingRule).toHaveBeenCalledWith(
        "route-active",
        expect.objectContaining({ id: target.id, outbound: "proxy" }),
      ),
    );
  });

  afterEach(() => {
    clients.forEach((client) => client.clear());
    clients.clear();
  });

  it("edits the active rule set, listing the rules after the pinned per-app rule", async () => {
    const { result } = renderController();

    await waitFor(() => expect(result.current.activeRouting?.id).toBe("route-active"));
    expect(result.current.loading).toBe(false);
    expect(result.current.rules.map((rule) => rule.id)).toEqual(["rule-a", "rule-b", "rule-c"]);
    await waitFor(() => expect(result.current.nodeNames).toEqual(["Tokyo", "Osaka"]));
  });

  it("lists a per-app rule that is not pinned first like any other rule", async () => {
    ipcMocks.listRoutings.mockResolvedValue([{ ...active(), rules: [rule("rule-a"), perApp()] }]);
    const { result } = renderController();

    await waitFor(() =>
      expect(result.current.rules.map((item) => item.id)).toEqual(["rule-a", "rule-per-app"]),
    );
  });

  it("switches a rule, showing the requested state until the save settles", async () => {
    let finish!: (value: Routing_Serialize) => void;
    ipcMocks.saveRoutingRule.mockReturnValueOnce(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    const { client, result } = renderController();
    await waitFor(() => expect(result.current.rules).toHaveLength(3));
    const target = result.current.rules[0];

    let toggle!: Promise<void>;
    act(() => {
      toggle = result.current.toggleRule(target, false);
    });
    expect(result.current.pendingToggles.get("rule-a")).toBe(false);
    // A second switch of the same rule is ignored while the first is saving.
    await act(() => result.current.toggleRule(target, true));
    expect(ipcMocks.saveRoutingRule).toHaveBeenCalledTimes(1);
    expect(ipcMocks.saveRoutingRule).toHaveBeenCalledWith("route-active", {
      ...target,
      enabled: false,
    });

    const committed: Routing_Serialize = {
      ...active(),
      isActive: false,
      rules: active().rules.map((item) =>
        item.id === "rule-a" ? { ...item, enabled: false } : item,
      ),
    };
    await act(async () => {
      finish(committed);
      await toggle;
    });

    expect(result.current.pendingToggles.size).toBe(0);
    // The committed rule set shows at once and keeps the active flag it had.
    const cached = client.getQueryData<Routing_Serialize[]>(queryKeys.routings);
    expect(cached?.[0]).toMatchObject({ id: "route-active", isActive: true });
    expect(cached?.[1].id).toBe("route-other");
    expect(result.current.rules[0].enabled).toBe(false);
  });

  it("moves rules by one step or to an end, never above the pinned per-app rule", async () => {
    ipcMocks.moveRoutingRule.mockResolvedValue(active());
    const { result } = renderController();
    await waitFor(() => expect(result.current.rules).toHaveLength(3));
    const [first, second] = result.current.rules;

    act(() => result.current.moveRule(second, "top"));
    act(() => result.current.moveRule(second, "up"));
    act(() => result.current.moveRule(first, "down"));
    act(() => result.current.moveRule(first, "bottom"));

    await waitFor(() => expect(ipcMocks.moveRoutingRule).toHaveBeenCalledTimes(4));
    expect(ipcMocks.moveRoutingRule.mock.calls).toEqual([
      ["route-active", "rule-b", "position", 1],
      ["route-active", "rule-b", "up", null],
      ["route-active", "rule-a", "down", null],
      ["route-active", "rule-a", "bottom", null],
    ]);
  });

  it("moves a rule to the very top when no per-app rule is pinned", async () => {
    ipcMocks.listRoutings.mockResolvedValue([
      { ...active(), rules: [rule("rule-a"), rule("rule-b")] },
    ]);
    ipcMocks.moveRoutingRule.mockResolvedValue(active());
    const { result } = renderController();
    await waitFor(() => expect(result.current.rules).toHaveLength(2));

    act(() => result.current.moveRule(result.current.rules[1], "top"));

    await waitFor(() =>
      expect(ipcMocks.moveRoutingRule).toHaveBeenCalledWith("route-active", "rule-b", "top", null),
    );
  });

  it("turns a drag and drop into the backend's insertion slot", async () => {
    ipcMocks.moveRoutingRule.mockResolvedValue(active());
    const { result } = renderController();
    await waitFor(() => expect(result.current.rules).toHaveLength(3));

    let committed = false;
    await act(async () => {
      committed = await result.current.reorderRule("rule-a", 0, 2);
    });
    expect(committed).toBe(true);
    // List index 0 → 2 is rule set index 1 → 3; the slot still counts the
    // moving rule, so it is 4.
    expect(ipcMocks.moveRoutingRule).toHaveBeenLastCalledWith(
      "route-active",
      "rule-a",
      "position",
      4,
    );

    await act(async () => {
      committed = await result.current.reorderRule("rule-c", 2, 0);
    });
    expect(ipcMocks.moveRoutingRule).toHaveBeenLastCalledWith(
      "route-active",
      "rule-c",
      "position",
      1,
    );

    ipcMocks.moveRoutingRule.mockRejectedValueOnce(new Error("move failed"));
    await act(async () => {
      committed = await result.current.reorderRule("rule-b", 1, 0);
    });
    expect(committed).toBe(false);
    expect(result.current.operationError).toBe("move failed");
  });

  it("saves a rule and closes the editor only when the backend accepts it", async () => {
    ipcMocks.saveRoutingRule
      .mockResolvedValueOnce(active())
      .mockRejectedValueOnce(new Error("rule save failed"));
    const { result } = renderController();
    await waitFor(() => expect(result.current.rules).toHaveLength(3));
    const payload = rule("rule-new");

    act(() => result.current.openCreateRule());
    expect(result.current.ruleDialog).toEqual({ mode: "create" });
    await act(() => result.current.saveRule(payload));
    expect(ipcMocks.saveRoutingRule).toHaveBeenCalledWith("route-active", payload);
    expect(result.current.ruleDialog).toBeNull();
    expect(result.current.operationError).toBeNull();

    const existing = result.current.rules[0];
    act(() => result.current.editRule(existing));
    expect(result.current.ruleDialog).toEqual({ mode: "edit", rule: existing });
    await act(() => result.current.saveRule(payload));
    // The refusal goes to the editor that is still open, not behind it.
    expect(result.current.operationError).toBeNull();
    expect(result.current.ruleSaveFailure).toEqual({ issues: [], message: "rule save failed" });
    expect(result.current.ruleDialog?.mode).toBe("edit");

    const issues = [{ code: { code: "invalidPort" }, field: "port", scope: [] }];
    ipcMocks.saveRoutingRule.mockRejectedValueOnce(
      Object.assign(new Error("invalid rule"), {
        appError: { kind: { issues, type: "validation" }, message: "invalid rule", subsystem: "app" },
      }),
    );
    await act(() => result.current.saveRule(payload));
    expect(result.current.ruleSaveFailure).toEqual({ issues, message: "invalid rule" });

    act(() => result.current.setRuleDialog(null));
    expect(result.current.ruleSaveFailure).toBeNull();
  });

  it("edits the per-app rule through its own dialog", async () => {
    const { result } = renderController();
    await waitFor(() => expect(result.current.rules).toHaveLength(3));

    act(() => result.current.editRule(perApp()));
    expect(useShellStore.getState().routingPerAppRequested).toBe(true);
    expect(result.current.ruleDialog).toBeNull();

    act(() => result.current.setPerAppOpen(false));
    expect(useShellStore.getState().routingPerAppRequested).toBe(false);
  });

  it("deletes a rule and restores the defaults only after confirmation", async () => {
    ipcMocks.deleteRoutingRules.mockResolvedValue(active());
    ipcMocks.resetRoutingRules.mockResolvedValue(active());
    const { result } = renderController();
    await waitFor(() => expect(result.current.rules).toHaveLength(3));
    const target = result.current.rules[1];

    act(() => result.current.requestDeleteRule(target));
    expect(result.current.pendingConfirm).toEqual({ kind: "deleteRule", rule: target });
    expect(ipcMocks.deleteRoutingRules).not.toHaveBeenCalled();
    act(() => result.current.confirmPending());
    await waitFor(() =>
      expect(ipcMocks.deleteRoutingRules).toHaveBeenCalledWith("route-active", ["rule-b"]),
    );
    expect(result.current.pendingConfirm).toBeNull();

    act(() => result.current.requestResetRules());
    act(() => result.current.setPendingConfirm(null));
    act(() => result.current.confirmPending());
    expect(ipcMocks.resetRoutingRules).not.toHaveBeenCalled();

    act(() => result.current.requestResetRules());
    expect(result.current.pendingConfirm).toEqual({ kind: "resetRules" });
    act(() => result.current.confirmPending());
    await waitFor(() => expect(ipcMocks.resetRoutingRules).toHaveBeenCalledWith("route-active"));
  });

  it("changes nothing while no rule set is active", async () => {
    ipcMocks.listRoutings.mockResolvedValue([inactive()]);
    const { result } = renderController();
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.activeRouting).toBeNull();
    expect(result.current.rules).toEqual([]);

    let committed = true;
    await act(() => result.current.saveRule(rule("rule-new")));
    await act(() => result.current.toggleRule(rule("rule-new"), false));
    await act(async () => {
      committed = await result.current.reorderRule("rule-new", 0, 1);
    });
    act(() => {
      result.current.moveRule(rule("rule-new"), "up");
      result.current.requestResetRules();
    });
    act(() => result.current.confirmPending());

    expect(committed).toBe(false);
    for (const mock of [
      ipcMocks.saveRoutingRule,
      ipcMocks.moveRoutingRule,
      ipcMocks.resetRoutingRules,
    ]) {
      expect(mock).not.toHaveBeenCalled();
    }
  });

  it("reports a rule set that fails to load", async () => {
    ipcMocks.listRoutings.mockRejectedValue(new Error("database locked"));
    const { result } = renderController();

    await waitFor(() => expect(result.current.loadError).toBe("database locked"));
    expect(result.current.activeRouting).toBeNull();
  });
});

function renderController() {
  const client = new QueryClient({ defaultOptions: { queries: { gcTime: 0, retry: false } } });
  clients.add(client);
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { client, ...renderHook(() => useRoutingScreen(), { wrapper }) };
}

function active(): Routing_Serialize {
  return routing("route-active", true, [perApp(), rule("rule-a"), rule("rule-b"), rule("rule-c")]);
}

function inactive(): Routing_Serialize {
  return routing("route-other", false, [rule("rule-x")]);
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

function perApp(): RoutingRule {
  return { ...rule("rule-per-app"), process: ["curl"], remarks: "voya:per-app-proxy" };
}

function rule(id: string): RoutingRule {
  return {
    domain: ["example.test"],
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
  };
}

function entry(remarks: string): ProfileListEntry {
  return { profile: { remarks } } as ProfileListEntry;
}
