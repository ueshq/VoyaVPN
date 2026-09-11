import { IpcCommandError, tunRequestElevation } from "@/ipc/commands";
import type { MissingCorePayload } from "@/stores/modal-store";

/**
 * Shared runtime-action helpers used by the Home hero and the node picker. Both
 * surfaces drive the same connect/restart IPC and react to the same elevation /
 * `missingCore` failures, so the handling lives here instead of being duplicated.
 *
 * The three `statusTo*` converters that used to live here are gone: the
 * transient `coreState` / `sysProxyChanged` / `tunChanged` events now carry the
 * same `RuntimeStatusResponse` / `SystemProxyStatusResponse` / `TunStatus` the
 * commands return, so a command result goes straight into the store.
 */

/**
 * A connect/restart failed because the machine needs one-time system
 * authorization first.
 *
 * The backend says so with a kind, never with a sentence. This used to also
 * accept any message containing "authorization", which matched the two texts it
 * was aimed at *and* `native authorization was cancelled` — so declining the
 * dialog re-opened it and re-ran the action — and would have matched any future
 * message that happened to use the word.
 */
function isElevationRequiredError(error: unknown) {
  return error instanceof IpcCommandError && error.appError.kind.type === "elevationRequired";
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
  if (!(error instanceof IpcCommandError) || error.appError.kind.type !== "missingCore") {
    return null;
  }

  return { coreType: error.appError.kind.coreType, message: error.appError.message };
}
