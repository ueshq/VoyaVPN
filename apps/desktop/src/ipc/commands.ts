import type { CommandResult } from "@voya/client/errors";
import { unwrapCommandResult } from "@voya/client/errors";
import type { VoyaCommands } from "@voya/contracts";

import { commands as rawCommands } from "@/ipc/bindings";

/**
 * The desktop's implementation of the whole `VoyaCommands` contract.
 *
 * One mapped object rather than one export per command: completeness is the
 * invariant (the `satisfies` in `register-backend.ts` enforces it), and a new
 * binding command appears here the moment it is generated.
 */
export const ipcCommands = Object.fromEntries(
  Object.entries(rawCommands).map(([key, command]) => [
    key,
    wrapCommand(command as (...args: unknown[]) => Promise<CommandResult<unknown>>),
  ]),
) as VoyaCommands;

function wrapCommand<Args extends unknown[], T>(
  command: (...args: Args) => Promise<CommandResult<T>>,
) {
  return async (...args: Args): Promise<T> => unwrapCommandResult(await command(...args));
}
