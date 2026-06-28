import { IpcCommandError } from "@/ipc";
import type { CoreStateEvent, RuntimeStatusResponse } from "@/ipc/bindings";
import type { MissingCorePayload } from "@/stores/modal-store";

/**
 * Shared runtime-action helpers used by the Home hero and the node picker. Both
 * surfaces drive the same connect/restart IPC and react to the same `sudo` /
 * `missingCore` failures, so the mapping lives here instead of being duplicated.
 */
export function statusToCoreState(status: RuntimeStatusResponse): CoreStateEvent {
  return {
    activeProfileId: status.activeProfileId,
    mainPid: status.mainPid,
    prePid: status.prePid,
    runningCoreType: status.runningCoreType,
    state: status.state,
  };
}

export function shouldOpenSudoPrompt(error: unknown) {
  if (!(error instanceof IpcCommandError)) {
    return false;
  }

  return error.appError.kind === "sudo" || error.message.toLowerCase().includes("sudo password");
}

export function missingCorePayload(error: unknown): MissingCorePayload | null {
  if (!(error instanceof IpcCommandError) || error.appError.kind !== "missingCore") {
    return null;
  }

  const missingCore = error.appError.message;

  return { coreType: missingCore.coreType, message: missingCore.message };
}
