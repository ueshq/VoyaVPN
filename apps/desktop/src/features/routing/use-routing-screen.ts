import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import {
  deleteRoutingRules,
  listProfiles,
  listRoutings,
  moveRoutingRule,
  resetRoutingRules,
  saveRoutingRule,
} from "@/ipc/commands";
import type { MoveAction, RoutingRule, Routing_Serialize } from "@/ipc/bindings";
import { profilesQueryKey, queryKeys } from "@/ipc/query-keys";
import { useShellStore } from "@/stores/shell-store";
import { getErrorMessage } from "@voya/utils/error";

import { nodeOutboundNames } from "./rule-outbound";
import type { RoutingRulePayload } from "./routing-form-schema";
import { PER_APP_SENTINEL } from "./sentinel-rules";

export type RuleMoveAction = Extract<MoveAction, "bottom" | "down" | "top" | "up">;

type RuleDialogState = { mode: "create" } | { mode: "edit"; rule: RoutingRule } | null;

/**
 * Deleting a rule and restoring the defaults cannot be undone, so both wait
 * for a confirmation that says what is about to change.
 */
type PendingConfirm = { kind: "deleteRule"; rule: RoutingRule } | { kind: "resetRules" } | null;

const NO_RULES: readonly RoutingRule[] = [];

function setPerAppOpen(open: boolean) {
  useShellStore.setState({ routingPerAppRequested: open });
}

/**
 * The Rules page edits the active rule set only: it is the one the core runs
 * with, and the page does not offer other rule sets to choose from.
 */
export function useRoutingScreen() {
  const client = useQueryClient();
  const [operationError, setOperationError] = useState<string | null>(null);
  const [ruleDialog, setRuleDialog] = useState<RuleDialogState>(null);
  const [pendingConfirm, setPendingConfirm] = useState<PendingConfirm>(null);
  const [pendingToggles, setPendingToggles] = useState<ReadonlyMap<string, boolean>>(
    () => new Map(),
  );
  const routingsQuery = useQuery({
    queryFn: listRoutings,
    queryKey: queryKeys.routings,
  });
  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, null),
    queryKey: profilesQueryKey(""),
  });

  const activeRouting = routingsQuery.data?.find((routing) => routing.isActive) ?? null;
  // The per-app dialog keeps its rule first so it matches ahead of everything
  // else. The page shows that rule as its own card, so the sortable list starts
  // after it and every position sent to the backend is shifted past it.
  const offset = activeRouting?.rules[0]?.remarks === PER_APP_SENTINEL ? 1 : 0;
  const rules = useMemo(
    () => activeRouting?.rules.slice(offset) ?? NO_RULES,
    [activeRouting, offset],
  );
  const profileEntries = profilesQuery.data?.entries;
  const nodeNames = useMemo(
    () => (profileEntries ? nodeOutboundNames(profileEntries) : null),
    [profileEntries],
  );

  function primeRouting(saved: Routing_Serialize) {
    // Every routing command answers with the rule set it committed. Writing it
    // into the cache shows the result at once; the backend's `routings`
    // invalidation still refetches the list afterwards.
    client.setQueryData<Routing_Serialize[]>(queryKeys.routings, (current) =>
      current?.map((routing) =>
        routing.id === saved.id ? { ...saved, isActive: routing.isActive } : routing,
      ),
    );
  }

  async function runOperation(
    operation: (routingId: string) => Promise<Routing_Serialize>,
  ): Promise<boolean> {
    if (!activeRouting) {
      return false;
    }
    setOperationError(null);
    try {
      primeRouting(await operation(activeRouting.id));
      return true;
    } catch (error) {
      setOperationError(getErrorMessage(error));
      return false;
    }
  }

  async function saveRule(rule: RoutingRulePayload) {
    if (await runOperation((routingId) => saveRoutingRule(routingId, rule))) {
      setRuleDialog(null);
    }
  }

  async function toggleRule(rule: RoutingRule, enabled: boolean) {
    if (!activeRouting || pendingToggles.has(rule.id)) {
      return;
    }
    // The switch shows the requested state while the save (and the core
    // restart it triggers) is in flight.
    setPendingToggles((current) => new Map(current).set(rule.id, enabled));
    try {
      await runOperation((routingId) => saveRoutingRule(routingId, { ...rule, enabled }));
    } finally {
      setPendingToggles((current) => {
        const next = new Map(current);
        next.delete(rule.id);
        return next;
      });
    }
  }

  function moveRule(rule: RoutingRule, action: RuleMoveAction) {
    void runOperation((routingId) =>
      // A plain "top" would lift the rule above the pinned per-app rule.
      action === "top" && offset > 0
        ? moveRoutingRule(routingId, rule.id, "position", offset)
        : moveRoutingRule(routingId, rule.id, action, null),
    );
  }

  /** Moves a rule from one list index to another, as a drag and drop does. */
  function reorderRule(ruleId: string, from: number, to: number) {
    const source = from + offset;
    const target = to + offset;
    // `position` is an insertion slot in the list as it was before the move.
    const position = target > source ? target + 1 : target;

    return runOperation((routingId) => moveRoutingRule(routingId, ruleId, "position", position));
  }

  function editRule(rule: RoutingRule) {
    if (rule.remarks === PER_APP_SENTINEL) {
      setPerAppOpen(true);
      return;
    }
    setRuleDialog({ mode: "edit", rule });
  }

  function confirmPending() {
    const pending = pendingConfirm;
    setPendingConfirm(null);
    if (!pending) {
      return;
    }
    void runOperation((routingId) =>
      pending.kind === "resetRules"
        ? resetRoutingRules(routingId)
        : deleteRoutingRules(routingId, [pending.rule.id]),
    );
  }

  return {
    activeRouting,
    confirmPending,
    editRule,
    loadError: routingsQuery.error ? getErrorMessage(routingsQuery.error) : null,
    loading: routingsQuery.isPending,
    moveRule,
    nodeNames,
    openCreateRule: () => setRuleDialog({ mode: "create" }),
    operationError,
    pendingConfirm,
    pendingToggles,
    reorderRule,
    requestDeleteRule: (rule: RoutingRule) => setPendingConfirm({ kind: "deleteRule", rule }),
    requestResetRules: () => setPendingConfirm({ kind: "resetRules" }),
    ruleDialog,
    rules,
    saveRule,
    setPendingConfirm,
    setPerAppOpen,
    setRuleDialog,
    toggleRule,
  };
}

export type RoutingScreenController = ReturnType<typeof useRoutingScreen>;
