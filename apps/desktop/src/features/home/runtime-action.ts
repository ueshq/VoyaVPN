import { IpcCommandError, tunRequestElevation } from "@/ipc";
import type {
  CoreStateEvent,
  RuntimeStatusResponse,
  SysProxyChanged,
  SystemProxyStatusResponse,
  TunChanged,
  TunStatus,
} from "@/ipc/bindings";
import type { MissingCorePayload } from "@/stores/modal-store";

/**
 * Shared runtime-action helpers used by the Home hero and the node picker. Both
 * surfaces drive the same connect/restart IPC and react to the same elevation /
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

export function statusToSysProxyChanged(status: SystemProxyStatusResponse): SysProxyChanged {
  return {
    effectiveMode: status.effectiveMode,
    pacAvailable: status.pacAvailable,
    proxy: status.proxy,
    requestedMode: status.requestedMode,
  };
}

export function statusToTunChanged(status: TunStatus): TunChanged {
  return {
    backend: status.backend,
    enabled: status.enabled,
    lastProviderError: status.lastProviderError,
    nativeComponentReady: status.nativeComponentReady,
    providerState: status.providerState,
  };
}

/** A connect/restart failed because TUN needs one-time system authorization. */
function isElevationRequiredError(error: unknown) {
  if (!(error instanceof IpcCommandError)) {
    return false;
  }

  return error.appError.kind === "sudo" || error.message.toLowerCase().includes("authorization");
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
    if (!isElevationRequiredError(error)) {
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
  if (!(error instanceof IpcCommandError) || error.appError.kind !== "missingCore") {
    return null;
  }

  const missingCore = error.appError.message;

  return { coreType: missingCore.coreType, message: missingCore.message };
}
