import { useRef, useState } from "react";
import { updateSubscriptions, deleteSubscriptions } from "@/ipc/commands";
import type { Subscription } from "@/ipc/bindings";
import {
  assertSubscriptionUpdated,
  formatSubscriptionUpdateSummary,
} from "@/features/subscriptions/subscription-update-result";
import type { TranslationFunction } from "@voya/i18n";
import type { NodeOperation } from "./use-node-operation";

/**
 * Pending-action markers for the one re-entrancy guard: a subscription id
 * while that subscription updates, `UPDATING_ALL` while they all do, and
 * `DELETING` while the confirm dialog's request is in flight. Backend ids are
 * `sub-`-prefixed uuids, so the sentinels cannot collide with them.
 */
const UPDATING_ALL = "all";
const DELETING = "delete";

export function useNodeSubscriptions(
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
  const subscriptionTriggerRef = useRef<HTMLElement | null>(null);

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

  function openSubscription(
    subscription: Subscription | null,
    trigger?: HTMLElement,
  ) {
    subscriptionTriggerRef.current =
      trigger ??
      (document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null);
    setEditingSubscription(subscription);
    setSubscriptionsOpen(true);
  }

  async function runSubscriptionUpdate(id: string | null) {
    if (!claimPending(id ?? UPDATING_ALL)) return;
    try {
      await runOperation(async () => {
        const result = await updateSubscriptions(id, true, null);
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
        await runOperation(() => deleteSubscriptions([deletingSubscription.id]))
      ) {
        setDeletingSubscription(null);
      }
    } finally {
      releasePending(DELETING);
    }
  }
  function confirmSubscriptionDeletion(
    subscription: Subscription,
    trigger: HTMLElement,
  ) {
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
