import { useQueryClient } from "@tanstack/react-query";

import type { PolicyGroupListing, RuntimeStatusResponse } from "@voya/contracts";
import type { TranslationFunction } from "@voya/i18n";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import { appErrorOfKind } from "./errors";
import { useModalStore, type MissingCorePayload } from "./modal-store";
import { requestElevation } from "./platform";
import { queryKeys } from "./query-keys";
import {
  runtimeActionPending,
  type RuntimeAction,
  useRuntimeActionStore,
} from "./runtime-action-store";
import { coreStateOf, runningProfileId, useRuntimeEventStore } from "./runtime-event-store";
import { beginRuntimeRead } from "./runtime-state-version";
import { refreshRuntimeStatusAndReport } from "./runtime-status";
import { toastError, useToastStore } from "./toast-store";
import { voyaCommands } from "./transport";

/** Apply a command response only while no newer read or event has superseded it. */
export async function executeRuntimeAction(action: RuntimeAction) {
  const isLatest = beginRuntimeRead("coreState");
  const commands = voyaCommands();
  // Called through the binding rather than passed as a detached reference: a
  // native transport is free to implement its surface as methods.
  const command = action === "connect" ? () => commands.connectActiveProfile()
    : action === "disconnect" ? () => commands.disconnectCore() : () => commands.restartCore();
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
  if (switchBusy(state)) {
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
  const busy = useSwitchBusy();
  const runningId = runningProfileId(coreState);

  function activateProfile(id: string) {
    return activateSelection(id, t, async () => {
      // A node replaces the policy group in use; say which one it set aside.
      const replacedGroup = queryClient
        .getQueryData<PolicyGroupListing>(queryKeys.policyGroups)
        ?.entries.find((entry) => entry.isActive)?.group.name;
      await voyaCommands().setActiveProfile(id);
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
function reportRuntimeActionError(
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
 * The reactive form of [`runtimeBusy`], for rendering: every busy indicator on
 * every screen derives from these two subscriptions, so they cannot drift.
 */
export function useRuntimeBusy() {
  const transitioning = useRuntimeEventStore((state) =>
    isRuntimeTransitioning(coreStateOf(state.coreState)),
  );
  const pending = useRuntimeActionStore(runtimeActionPending);
  return transitioning || pending;
}

/**
 * Node and group switches also wait out a pending TUN cleanup, which runs
 * after a disconnect and would race the switch's own connect.
 */
function switchBusy(state: RuntimeStatusResponse["state"]) {
  return runtimeBusy(state) || state === "cleanupPending";
}

/** The reactive form of [`switchBusy`], for rendering a switch button's busy state. */
function useSwitchBusy() {
  const cleaning = useRuntimeEventStore(
    (state) => coreStateOf(state.coreState) === "cleanupPending",
  );
  return useRuntimeBusy() || cleaning;
}

/**
 * Run a runtime action; if it fails only because the tunnel needs system
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
    if (!(await requestElevation())) {
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
