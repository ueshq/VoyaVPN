import type { TranslationFunction } from "@voya/i18n";
import { getErrorMessage } from "@voya/utils/error";
import { connectActiveProfile, disconnectCore, IpcCommandError, restartCore, tunRequestElevation } from "@/ipc/commands";
import type { RuntimeStatusResponse } from "@/ipc/bindings";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { beginRuntimeRead } from "@/ipc/runtime-state-version";
import { useModalStore, type MissingCorePayload } from "@/stores/modal-store";
import type { RuntimeAction } from "@/stores/runtime-action-store";
import { useToastStore } from "@/stores/toast-store";

/** Apply a command response only while no newer read or event has superseded it. */
export async function executeRuntimeAction(action: RuntimeAction) {
  const isLatest = beginRuntimeRead("coreState");
  const command = action === "connect" ? connectActiveProfile
    : action === "disconnect" ? disconnectCore : restartCore;
  const status = await runWithElevation(command);
  if (isLatest()) useRuntimeEventStore.getState().setCoreState(status);
  return status;
}

export function reportRuntimeActionError(error: unknown, action: RuntimeAction, t: TranslationFunction) {
  const missingCore = missingCorePayload(error);
  if (missingCore) {
    useModalStore.getState().openModal("missingCore", { missingCore });
  } else {
    useToastStore.getState().pushToast({
      description: getErrorMessage(error),
      severity: "error",
      title: {
        connect: t("actions.connect"),
        disconnect: t("actions.disconnect"),
        restart: t("actions.restart"),
      }[action],
    });
  }
}

export function isRuntimeTransitioning(state: RuntimeStatusResponse["state"]) {
  return state === "connecting" || state === "disconnecting";
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
    if (!(error instanceof IpcCommandError) || error.appError.kind.type !== "elevationRequired") {
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
  if (!(error instanceof IpcCommandError) || error.appError.kind.type !== "missingCore") {
    return null;
  }

  return { coreType: error.appError.kind.coreType, message: error.appError.message };
}
