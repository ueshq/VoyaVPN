import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import {
  deleteRoutingRules,
  listPolicyGroups,
  listProfiles,
  listRoutings,
  moveRoutingRule,
  resetRoutingRules,
  saveRoutingRule,
} from "@/ipc/commands";
import type {
  AppError,
  MoveAction,
  RoutingRule,
  Routing_Serialize,
  ValidationIssue,
} from "@/ipc/bindings";
import { profilesQueryKey, queryKeys } from "@/ipc/query-keys";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";
import { useI18n } from "@voya/i18n/use-i18n";
import { getErrorMessage } from "@voya/utils/error";

import { nodeOutboundNames, type RuleGroupOutbound } from "./rule-outbound";
import type { RoutingRulePayload } from "./routing-form-schema";
import { PER_APP_SENTINEL } from "./sentinel-rules";

export type RuleMoveAction = Extract<MoveAction, "bottom" | "down" | "top" | "up">;

type RuleDialogState = { mode: "create" } | { mode: "edit"; rule: RoutingRule } | null;

/**
 * Deleting a rule, restoring the defaults and pointing a rule whose node is
 * gone at the proxy all change what rules do, so each waits for a confirmation
 * that says what is about to change.
 */
type PendingConfirm =
  | { kind: "deleteRule"; rule: RoutingRule }
  | { kind: "fixOutbound"; rule: RoutingRule }
  | { kind: "resetRules" }
  | null;

const NO_RULES: readonly RoutingRule[] = [];

/** Why the backend refused a rule, kept for the editor that is still open. */
export type RuleSaveError = { issues: readonly ValidationIssue[]; message: string };

function ruleSaveError(error: unknown): RuleSaveError {
  const kind = (error as { appError?: AppError } | null)?.appError?.kind;
  return {
    issues: kind?.type === "validation" ? kind.issues : [],
    message: getErrorMessage(error),
  };
}

function setPerAppOpen(open: boolean) {
  useShellStore.setState({ routingPerAppRequested: open });
}

/**
 * The Rules page edits the active rule set only: it is the one the core runs
 * with, and the page does not offer other rule sets to choose from.
 */
export function useRoutingScreen() {
  const { t } = useI18n();
  const client = useQueryClient();
  const [operationError, setOperationError] = useState<string | null>(null);
  const [ruleDialog, setRuleDialog] = useState<RuleDialogState>(null);
  const [ruleSaveFailure, setRuleSaveFailure] = useState<RuleSaveError | null>(null);
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
  const policyGroupsQuery = useQuery({
    queryFn: listPolicyGroups,
    queryKey: queryKeys.policyGroups,
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
  const groupEntries = policyGroupsQuery.data?.entries;
  const groupOutbounds = useMemo<RuleGroupOutbound[] | null>(
    () => groupEntries?.map(({ group }) => ({ id: group.id, name: group.name })) ?? null,
    [groupEntries],
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
    onError: (error: unknown) => void = (error) => setOperationError(getErrorMessage(error)),
  ): Promise<boolean> {
    if (!activeRouting) {
      return false;
    }
    setOperationError(null);
    try {
      primeRouting(await operation(activeRouting.id));
      return true;
    } catch (error) {
      onError(error);
      return false;
    }
  }

  function changeRuleDialog(next: RuleDialogState) {
    setRuleSaveFailure(null);
    setRuleDialog(next);
  }

  async function saveRule(rule: RoutingRulePayload) {
    setRuleSaveFailure(null);
    // The editor is still open over the page, so a refusal is reported there;
    // the page's own error strip would sit behind the modal.
    const saved = await runOperation(
      (routingId) => saveRoutingRule(routingId, rule),
      (error) => setRuleSaveFailure(ruleSaveError(error)),
    );
    if (saved) {
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

    // The dragged row snaps back on failure. A toast says why, since the page's
    // error strip may be out of view above a long list.
    return runOperation(
      (routingId) => moveRoutingRule(routingId, ruleId, "position", position),
      (error) =>
        useToastStore.getState().pushToast({
          description: getErrorMessage(error),
          severity: "error",
          title: t("panes.routing.reorderFailed"),
        }),
    );
  }

  function editRule(rule: RoutingRule) {
    if (rule.remarks === PER_APP_SENTINEL) {
      setPerAppOpen(true);
      return;
    }
    changeRuleDialog({ mode: "edit", rule });
  }

  function confirmPending() {
    const pending = pendingConfirm;
    setPendingConfirm(null);
    if (!pending) {
      return;
    }
    void runOperation((routingId) => {
      switch (pending.kind) {
        case "resetRules":
          return resetRoutingRules(routingId);
        case "deleteRule":
          return deleteRoutingRules(routingId, [pending.rule.id]);
        case "fixOutbound":
          // The node or group the rule named is gone; the proxy always exists.
          return saveRoutingRule(routingId, { ...pending.rule, outbound: "proxy" });
      }
    });
  }

  return {
    activeRouting,
    confirmPending,
    editRule,
    groupOutbounds,
    loadError: routingsQuery.error ? getErrorMessage(routingsQuery.error) : null,
    loading: routingsQuery.isPending,
    moveRule,
    nodeNames,
    openCreateRule: () => changeRuleDialog({ mode: "create" }),
    operationError,
    pendingConfirm,
    pendingToggles,
    reorderRule,
    requestDeleteRule: (rule: RoutingRule) => setPendingConfirm({ kind: "deleteRule", rule }),
    /** Points a rule whose node or group is gone back at the proxy, once confirmed. */
    requestFixOutbound: (rule: RoutingRule) => setPendingConfirm({ kind: "fixOutbound", rule }),
    requestResetRules: () => setPendingConfirm({ kind: "resetRules" }),
    ruleDialog,
    ruleSaveFailure,
    rules,
    saveRule,
    setPendingConfirm,
    setPerAppOpen,
    setRuleDialog: changeRuleDialog,
    toggleRule,
  };
}

export type RoutingScreenController = ReturnType<typeof useRoutingScreen>;
