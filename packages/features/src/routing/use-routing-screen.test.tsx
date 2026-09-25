import { act, waitFor } from "@testing-library/react";
import type { QueryClient } from "@tanstack/react-query";
import { createTestQueryClient, renderHookWithQuery } from "../test/render";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ProfileSummaryEntry, RoutingRule, Routing_Serialize, ValidationIssue } from "@voya/contracts";
import { IpcCommandError } from "@voya/client/errors";
import { queryKeys } from "@voya/client/query-keys";
import { useToastStore } from "@voya/client/toast-store";

import { installFakeCommands } from "../test/backend";

import { useRoutingScreen } from "./use-routing-screen";

// The hook reaches the backend through the shared seam, and the error class
// and kind check stay real, so a refused rule keeps its issues.
const ipcMocks = installFakeCommands({
  deleteRoutingRules: vi.fn(),
  listPolicyGroups: vi.fn(),
  listProfileSummaries: vi.fn(),
  listRoutings: vi.fn(),
  moveRoutingRule: vi.fn(),
  resetRoutingRules: vi.fn(),
  saveRoutingRule: vi.fn(),
});

const clients = new Set<QueryClient>();

describe("useRoutingScreen", () => {
  beforeEach(() => {
    Object.values(ipcMocks).forEach((mock) => mock.mockReset());
    ipcMocks.listRoutings.mockResolvedValue([active(), inactive()]);
    ipcMocks.listProfileSummaries.mockResolvedValue({
      entries: [entry("Tokyo"), entry("Tokyo"), entry("Osaka")],
      undecodableProfiles: 0,
    });
    ipcMocks.listPolicyGroups.mockResolvedValue({ entries: [] });
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
    await waitFor(() => expect(result.current.nodeNames).toEqual(new Set(["Tokyo", "Osaka"])));
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
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "move failed",
      severity: "error",
      title: "Could not reorder the rules",
    });
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

    const issues: ValidationIssue[] = [{ code: { code: "invalidPort" }, field: "port", scope: [] }];
    ipcMocks.saveRoutingRule.mockRejectedValueOnce(
      new IpcCommandError({ kind: { issues, type: "validation" }, message: "invalid rule", subsystem: "app" }),
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
    expect(result.current.perAppOpen).toBe(true);
    expect(result.current.ruleDialog).toBeNull();

    act(() => result.current.setPerAppOpen(false));
    expect(result.current.perAppOpen).toBe(false);
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
  const client = createTestQueryClient({ gcTime: 0 });
  clients.add(client);
  return { client, ...renderHookWithQuery(() => useRoutingScreen(), { queryClient: client }) };
}

function active(): Routing_Serialize {
  return routing("route-active", true, [perApp(), rule("rule-a"), rule("rule-b"), rule("rule-c")]);
}

function inactive(): Routing_Serialize {
  return routing("route-other", false, [rule("rule-x")]);
}

function routing(id: string, isActive: boolean, rules: RoutingRule[]): Routing_Serialize {
  return {
    id,
    isActive,
    remarks: id,
    rules,
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

function entry(remarks: string): ProfileSummaryEntry {
  return { profile: { remarks } } as ProfileSummaryEntry;
}
