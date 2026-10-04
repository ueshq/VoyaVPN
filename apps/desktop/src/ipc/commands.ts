import type { CommandResult } from "@voya/client/errors";
import { unwrapCommandResult } from "@voya/client/errors";
import type { VoyaCommands } from "@voya/contracts";

import { commands as rawCommands } from "@/ipc/bindings";

/** The generated binding with each command's result envelope unwrapped. */
type Unwrapped<Raw> = {
  [Name in keyof Raw]: Raw[Name] extends (
    ...args: infer Args
  ) => Promise<CommandResult<infer Value>>
    ? (...args: Args) => Promise<Value>
    : never;
};

/**
 * The desktop's implementation of the whole `VoyaCommands` contract.
 *
 * One mapped object rather than one export per command: completeness is the
 * invariant, and the `satisfies` below enforces it — a command the contract
 * names and the generated binding lacks, or one whose arguments or result
 * drifted, fails to compile here.
 */
export const ipcCommands = (Object.fromEntries(
  Object.entries(rawCommands).map(([key, command]) => [
    key,
    wrapCommand(command as (...args: unknown[]) => Promise<CommandResult<unknown>>),
  ]),
) as Unwrapped<typeof rawCommands>) satisfies VoyaCommands;

function wrapCommand<Args extends unknown[], T>(
  command: (...args: Args) => Promise<CommandResult<T>>,
) {
  return async (...args: Args): Promise<T> => unwrapCommandResult(await command(...args));
}
