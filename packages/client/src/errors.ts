import type { AppError, AppErrorKind } from "@voya/contracts";

/**
 * A command that the backend rejected.
 *
 * Every transport rejects with this, so a feature branches on the typed
 * `kind` regardless of whether it reached the backend over Tauri IPC or a
 * native module.
 */
export class IpcCommandError extends Error {
  readonly appError: AppError;

  constructor(appError: AppError) {
    super(appError.message);
    this.appError = appError;
    this.name = "IpcCommandError";
  }
}

/** A rejected command's backend error when it is of the given kind, otherwise `null`. */
export function appErrorOfKind<Type extends AppErrorKind["type"]>(
  error: unknown,
  type: Type,
): (AppError & { kind: Extract<AppErrorKind, { type: Type }> }) | null {
  return error instanceof IpcCommandError && error.appError.kind.type === type
    ? (error.appError as AppError & { kind: Extract<AppErrorKind, { type: Type }> })
    : null;
}

export type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

/**
 * Unwraps the result envelope a transport returns, throwing the error arm.
 *
 * `message` is an English diagnostic: show it, log it, never parse it. Use
 * `appErrorOfKind` to branch.
 */
export function unwrapCommandResult<T>(result: CommandResult<T>): T {
  if (result.status === "error") {
    throw new IpcCommandError(result.error);
  }

  return result.data;
}
