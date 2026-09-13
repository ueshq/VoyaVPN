import { useRef, useState } from "react";
import { updateSubscriptions, deleteSubscriptions } from "@/ipc/commands";
import type { Subscription } from "@/ipc/bindings";
import {
  formatSubscriptionUpdateSummary,
  isSubscriptionUpdateFailure,
  subscriptionUpdateMessages,
} from "@/features/subscriptions/subscription-update-result";
import type { TranslationFunction } from "@voya/i18n";
import type { NodeOperation } from "./use-node-operation";

export function useNodeSubscriptions(
  { runOperation, setOperationError, setOperationMessage }: NodeOperation,
  t: TranslationFunction,
) {
  const [subscriptionsOpen, setSubscriptionsOpen] = useState(false);
  const [editingSubscription, setEditingSubscription] =
    useState<Subscription | null>(null);
  const [deletingSubscription, setDeletingSubscription] =
    useState<Subscription | null>(null);
  const [deletingSubscriptionPending, setDeletingSubscriptionPending] =
    useState(false);
  const deletingSubscriptionRef = useRef(false);
  const [updatingSubscriptions, setUpdatingSubscriptions] = useState<
    Set<string>
  >(() => new Set());
  const updatingRef = useRef(new Set<string>());
  const [updatingAllSubscriptions, setUpdatingAllSubscriptions] = useState(false);
  const updatingAllRef = useRef(false);
  const subscriptionTriggerRef = useRef<HTMLElement | null>(null);
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
  async function updateSubscription(id: string) {
    if (updatingRef.current.has(id)) return;
    updatingRef.current.add(id);
    setUpdatingSubscriptions(new Set(updatingRef.current));
    try {
      await runOperation(async () => {
        const result = await updateSubscriptions(id, true, null);
        if (isSubscriptionUpdateFailure(result))
          throw new Error(
            subscriptionUpdateMessages(result) ||
              t("panes.subscriptions.updateNothingImported"),
          );
        setOperationMessage(formatSubscriptionUpdateSummary(result, t));
      });
    } finally {
      updatingRef.current.delete(id);
      setUpdatingSubscriptions(new Set(updatingRef.current));
    }
  }
  async function updateAllSubscriptions() {
    if (updatingAllRef.current) return;
    updatingAllRef.current = true;
    setUpdatingAllSubscriptions(true);
    try {
      await runOperation(async () => {
        const result = await updateSubscriptions(null, true, null);
        if (isSubscriptionUpdateFailure(result))
          throw new Error(
            subscriptionUpdateMessages(result) ||
              t("panes.subscriptions.updateNothingImported"),
          );
        setOperationMessage(formatSubscriptionUpdateSummary(result, t));
      });
    } finally {
      updatingAllRef.current = false;
      setUpdatingAllSubscriptions(false);
    }
  }
  async function removeSubscription() {
    if (!deletingSubscription || deletingSubscriptionRef.current) return;
    deletingSubscriptionRef.current = true;
    setDeletingSubscriptionPending(true);
    try {
      if (
        await runOperation(() => deleteSubscriptions([deletingSubscription.id]))
      ) {
        setDeletingSubscription(null);
      }
    } finally {
      deletingSubscriptionRef.current = false;
      setDeletingSubscriptionPending(false);
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
    deletingSubscriptionPending,
    removeSubscription,
    confirmSubscriptionDeletion,
    openSubscription,
    updateAllSubscriptions,
    updateSubscription,
    updatingAllSubscriptions,
    updatingSubscriptions,
    subscriptionTriggerRef,
  };
}
