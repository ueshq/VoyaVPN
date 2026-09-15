import { useQueryClient } from "@tanstack/react-query";

import type { TranslationFunction } from "@voya/i18n";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import {
  appErrorOfKind,
  connectActiveProfile,
  disconnectCore,
  restartCore,
  setActiveProfile,
  tunRequestElevation,
} from "@/ipc/commands";
import type { PolicyGroupListing, RuntimeStatusResponse } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { coreStateOf, runningProfileId, useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { refreshRuntimeStatusAndReport } from "@/ipc/runtime-status";
import { beginRuntimeRead } from "@/ipc/runtime-state-version";
import { useModalStore, type MissingCorePayload } from "@/stores/modal-store";
import {
  runtimeActionPending,
  type RuntimeAction,
  useRuntimeActionStore,
} from "@/stores/runtime-action-store";
import { toastError, useToastStore } from "@/stores/toast-store";

/** Apply a command response only while no newer read or event has superseded it. */
export async function executeRuntimeAction(action: RuntimeAction) {
  const isLatest = beginRuntimeRead("coreState");
  const command = action === "connect" ? connectActiveProfile
    : action === "disconnect" ? disconnectCore : restartCore;
  const status = await runWithElevation(command);
  if (isLatest()) useRuntimeEventStore.getState().setCoreState(status);
  return status;
}

/**
 * Connect, disconnect or restart from any screen. Nothing happens while another
 * runtime action or a transition is under way; afterwards the status is read
 * back whatever the outcome. `inline` keeps a failure next to Home's button
 * instead of a toast.
 */
export async function runRuntimeAction(
  action: RuntimeAction,
  t: TranslationFunction,
  { inline = false }: { inline?: boolean } = {},
) {
  const state = coreStateOf(useRuntimeEventStore.getState().coreState);
  if (runtimeBusy(state)) return;

  const store = useRuntimeActionStore.getState();
  store.startAction(action);
  try {
    await executeRuntimeAction(action);
  } catch (error) {
    reportRuntimeActionError(error, action, t, { inline });
  } finally {
    try {
      await refreshRuntimeStatusAndReport(t);
    } finally {
      store.finishAction();
    }
  }
}

/**
 * Makes a node or policy group the active selection, then connects, or restarts
 * a running core, with it. `switchingId` marks what is being switched to in the
 * shared guard and `select` saves the choice. Resolves to whether the core ended
 * up connected.
 */
export async function activateSelection(
  switchingId: string,
  t: TranslationFunction,
  select: () => Promise<void>,
): Promise<boolean> {
  const state = coreStateOf(useRuntimeEventStore.getState().coreState);
  if (runtimeBusy(state) || state === "cleanupPending") {
    return false;
  }

  const store = useRuntimeActionStore.getState();
  store.startSwitch(switchingId);
  const action = state === "connected" ? "restart" : "connect";
  try {
    await select();
    const status = await executeRuntimeAction(action);
    return status.state === "connected";
  } catch (error) {
    reportRuntimeActionError(error, action, t);
    return false;
  } finally {
    try {
      await refreshRuntimeStatusAndReport(t);
    } finally {
      store.finishSwitch();
    }
  }
}

/** Choosing a node to use: the shared runtime guard, the running node and `activateProfile`. */
export function useProfileActivation(t: TranslationFunction) {
  const queryClient = useQueryClient();
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const switchingId = useRuntimeActionStore((state) => state.switchingId);
  const pending = useRuntimeActionStore(runtimeActionPending);
  const state = coreStateOf(coreState);
  const busy = pending || isRuntimeTransitioning(state) || state === "cleanupPending";
  const runningId = runningProfileId(coreState);

  function activateProfile(id: string) {
    return activateSelection(id, t, async () => {
      // A node replaces the policy group in use; say which one it set aside.
      const replacedGroup = queryClient
        .getQueryData<PolicyGroupListing>(queryKeys.policyGroups)
        ?.entries.find((entry) => entry.isActive)?.group.name;
      await setActiveProfile(id);
      if (replacedGroup) {
        useToastStore.getState().pushToast({
          description: t("policyGroups.replacedByNode", { group: replacedGroup }),
          severity: "info",
          title: t("policyGroups.switchedTitle"),
        });
      }
    });
  }

  return { activateProfile, busy, runningId, switchingId };
}

/**
 * Surface a failed runtime action. Home keeps it inline next to the button, with
 * a retry; other screens use a toast because the button is not on screen.
 */
export function reportRuntimeActionError(
  error: unknown,
  action: RuntimeAction,
  t: TranslationFunction,
  { inline = false }: { inline?: boolean } = {},
) {
  const missingCore = missingCorePayload(error);
  if (missingCore) {
    useModalStore.getState().showMissingCore(missingCore);
  } else if (inline) {
    useRuntimeActionStore.getState().failInline(action, runtimeActionMessage(error, t));
  } else {
    toastError(
      {
        connect: t("actions.connect"),
        disconnect: t("actions.disconnect"),
        restart: t("actions.restart"),
      }[action],
      runtimeActionMessage(error, t),
    );
  }
}

/**
 * An authorization error that survives `runWithElevation` means the system
 * prompt was declined: the user's choice, which deserves words, not a code.
 */
function runtimeActionMessage(error: unknown, t: TranslationFunction) {
  return appErrorOfKind(error, "elevationRequired")
    ? t("home.authorizationDeclined")
    : redactOperationalError(error);
}

export function isRuntimeTransitioning(state: RuntimeStatusResponse["state"]) {
  return state === "connecting" || state === "disconnecting";
}

/** A new runtime action waits while another one runs or the core is changing state. */
export function runtimeBusy(state: RuntimeStatusResponse["state"]) {
  return runtimeActionPending() || isRuntimeTransitioning(state);
}

/**
 * Run a runtime action; if it fails only because TUN needs system
 * authorization, request it once (native dialog, no stored password) and retry.
 * Other failures — and a cancelled dialog — rethrow the original error.
 */
export async function runWithElevation<T>(action: () => Promise<T>): Promise<T> {
  try {
    return await action();
  } catch (error) {
    if (!appErrorOfKind(error, "elevationRequired")) {
      throw error;
    }
    const status = await tunRequestElevation();
    if (!status.elevationGranted) {
      throw error;
    }
    return await action();
  }
}

export function missingCorePayload(error: unknown): MissingCorePayload | null {
  const missingCore = appErrorOfKind(error, "missingCore");

  return missingCore
    ? { message: missingCore.message }
    : null;
}
