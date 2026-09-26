import { useRef, useState } from "react";
import { voyaCommands } from "@voya/client/transport";
import type { Subscription, SubscriptionUpdateResult } from "@voya/contracts";
import { useMountedRef } from "@voya/utils/use-mounted-ref";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import {
  assertSubscriptionUpdated,
  subscriptionUpdateMessages,
  formatSubscriptionUpdateSummary,
} from "@voya/features/subscriptions/subscription-update-result";
import type { TranslationFunction } from "@voya/i18n/core";
import type { NodeOperation } from "@voya/features/profiles/use-node-operation";

/**
 * Pending-action markers for the one re-entrancy guard: a subscription id
 * while that subscription updates, `UPDATING_ALL` while they all do, and
 * `DELETING` while the confirm dialog's request is in flight. Backend ids are
 * `sub-`-prefixed uuids, so the sentinels cannot collide with them.
 */
const UPDATING_ALL = "all";
const DELETING = "delete";
const IMPORTED = "imported";

/**
 * `Trigger` is whatever the caller wants focus to return to once a dialog
 * closes — an `HTMLElement` on the desktop, nothing on a phone, where a sheet
 * dismisses back to the list on its own. It is carried, never inspected, so the
 * hook needs no DOM.
 */
export function useNodeSubscriptions<Trigger = never>(
  { runOperation, setOperationError, setOperationMessage }: NodeOperation,
  t: TranslationFunction,
) {
  const [subscriptionsOpen, setSubscriptionsOpen] = useState(false);
  const [editingSubscription, setEditingSubscription] =
    useState<Subscription | null>(null);
  const [deletingSubscription, setDeletingSubscription] =
    useState<Subscription | null>(null);
  const [pendingActions, setPendingActions] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const pendingActionsRef = useRef(new Set<string>());
  const subscriptionTriggerRef = useRef<Trigger | null>(null);
  const mounted = useMountedRef();

  function claimPending(key: string) {
    if (!mounted.current || pendingActionsRef.current.has(key)) return false;
    if (pendingActionsRef.current.has(UPDATING_ALL) || pendingActionsRef.current.has(IMPORTED)) return false;
    if ((key === UPDATING_ALL || key === IMPORTED) && pendingActionsRef.current.size > 0) return false;
    pendingActionsRef.current.add(key);
    if (mounted.current) setPendingActions(new Set(pendingActionsRef.current));
    return true;
  }

  function releasePending(key: string) {
    pendingActionsRef.current.delete(key);
    if (mounted.current) setPendingActions(new Set(pendingActionsRef.current));
  }

  function openSubscription(subscription: Subscription | null, trigger?: Trigger) {
    subscriptionTriggerRef.current = trigger ?? null;
    setEditingSubscription(subscription);
    setSubscriptionsOpen(true);
  }

  async function runSubscriptionUpdate(id: string | null) {
    if (!claimPending(id ?? UPDATING_ALL)) return;
    try {
      await runOperation(async () => {
        const result = await voyaCommands().updateSubscriptions(id, true, null);
        assertSubscriptionUpdated(result, t);
        setOperationMessage(formatSubscriptionUpdateSummary(result, t));
        const failure = subscriptionUpdateMessages(result, t);
        if (failure) setOperationError(failure);
      });
    } finally {
      releasePending(id ?? UPDATING_ALL);
    }
  }

  function updateSubscription(id: string) {
    return runSubscriptionUpdate(id);
  }

  function updateAllSubscriptions() {
    return runSubscriptionUpdate(null);
  }

  /** A source import is not complete until its nodes have been downloaded. */
  async function updateImportedSubscriptions(ids: string[], isActive: () => boolean) {
    const active = () => mounted.current && isActive();
    if (ids.length === 0 || !active() || !claimPending(IMPORTED)) return;
    const total: SubscriptionUpdateResult = {
      imported: 0, updated: 0, skipped: 0, removedExisting: 0, messages: [], outcomes: [],
    };
    const failures: string[] = [];
    setOperationError(null);
    setOperationMessage(t("panes.subscriptions.importUpdating"));
    try {
      for (const id of new Set(ids)) {
        if (!active()) return;
        try {
          const result = await voyaCommands().updateSubscriptions(id, true, null);
          assertSubscriptionUpdated(result, t);
          total.imported += result.imported;
          total.updated += result.updated;
          total.removedExisting += result.removedExisting;
          total.outcomes.push(...result.outcomes);
          const failure = subscriptionUpdateMessages(result, t);
          if (failure) failures.push(failure);
        } catch (error) {
          failures.push(redactOperationalError(error));
        }
      }
      if (!active()) return;
      setOperationMessage(total.updated > 0 || failures.length === 0 ? formatSubscriptionUpdateSummary(total, t) : null);
      if (failures.length > 0) {
        setOperationError(`${t("panes.subscriptions.importUpdateFailed")}\n${[...new Set(failures)].join("\n")}`);
      }
    } finally {
      releasePending(IMPORTED);
    }
  }

  async function removeSubscription() {
    if (!deletingSubscription || !claimPending(DELETING)) return;
    try {
      if (
        await runOperation(() => voyaCommands().deleteSubscriptions([deletingSubscription.id]))
      ) {
        setDeletingSubscription(null);
      }
    } finally {
      releasePending(DELETING);
    }
  }
  function confirmSubscriptionDeletion(subscription: Subscription, trigger: Trigger) {
    subscriptionTriggerRef.current = trigger;
    setOperationError(null);
    setDeletingSubscription(subscription);
  }

  return {
    subscriptionsOpen,
    setSubscriptionsOpen,
    editingSubscription,
    deletingSubscription,
    setDeletingSubscription,
    deletingSubscriptionPending: pendingActions.has(DELETING),
    removeSubscription,
    confirmSubscriptionDeletion,
    openSubscription,
    updateAllSubscriptions,
    updateSubscription,
    updateImportedSubscriptions,
    updatingAllSubscriptions: pendingActions.has(UPDATING_ALL) || pendingActions.has(IMPORTED),
    updatingSubscriptions: pendingActions,
    subscriptionTriggerRef,
  };
}
