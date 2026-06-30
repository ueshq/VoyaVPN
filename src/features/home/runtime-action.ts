import { IpcCommandError, tunRequestElevation } from "@/ipc";
import type {
  CoreStateEvent,
  RuntimeStatusResponse,
  SysProxyChanged,
  SysProxyMode,
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

/** Maps the string `SysProxyMode` to the numeric `SysProxyType` the backend expects. */
export const SYS_PROXY_TYPE = {
  forcedClear: 0,
  forcedChange: 1,
  unchanged: 2,
  pac: 3,
} as const satisfies Record<SysProxyMode, number>;

// The selector offers exactly three modes — off / smart / global — in this
// order. `unchanged` is intentionally not surfaced (the backend enum still
// supports it); PAC is always offered (it silently no-ops on the rare platform
// without PAC support).
export const PROXY_MODE_OPTIONS: SysProxyMode[] = ["forcedClear", "pac", "forcedChange"];

export function sysProxyTypeToMode(mode: number): SysProxyMode {
  switch (mode) {
    case SYS_PROXY_TYPE.forcedChange:
      return "forcedChange";
    case SYS_PROXY_TYPE.unchanged:
      return "unchanged";
    case SYS_PROXY_TYPE.pac:
      return "pac";
    case SYS_PROXY_TYPE.forcedClear:
    default:
      return "forcedClear";
  }
}

export function statusToSysProxyChanged(status: SystemProxyStatusResponse): SysProxyChanged {
  return {
    effectiveMode: sysProxyTypeToMode(status.effectiveMode),
    pacAvailable: status.pacAvailable,
    proxy: status.proxy,
    requestedMode: sysProxyTypeToMode(status.requestedMode),
  };
}

export function statusToTunChanged(status: TunStatus): TunChanged {
  return {
    enabled: status.enabled,
  };
}

/** A connect/restart failed because TUN needs one-time system authorization. */
export function isElevationRequiredError(error: unknown) {
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
