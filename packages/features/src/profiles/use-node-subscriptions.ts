import { useRef, useState } from "react";
import { voyaCommands } from "@voya/client/transport";
import type { Subscription } from "@voya/contracts";
import {
  assertSubscriptionUpdated,
  formatSubscriptionUpdateSummary,
} from "@voya/features/subscriptions/subscription-update-result";
import type { TranslationFunction } from "@voya/i18n";
import type { NodeOperation } from "@voya/features/profiles/use-node-operation";

/**
 * Pending-action markers for the one re-entrancy guard: a subscription id
 * while that subscription updates, `UPDATING_ALL` while they all do, and
 * `DELETING` while the confirm dialog's request is in flight. Backend ids are
 * `sub-`-prefixed uuids, so the sentinels cannot collide with them.
 */
const UPDATING_ALL = "all";
const DELETING = "delete";

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

  function claimPending(key: string) {
    if (pendingActionsRef.current.has(key)) return false;
    pendingActionsRef.current.add(key);
    setPendingActions(new Set(pendingActionsRef.current));
    return true;
  }

  function releasePending(key: string) {
    pendingActionsRef.current.delete(key);
    setPendingActions(new Set(pendingActionsRef.current));
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
    updatingAllSubscriptions: pendingActions.has(UPDATING_ALL),
    updatingSubscriptions: pendingActions,
    subscriptionTriggerRef,
  };
}
