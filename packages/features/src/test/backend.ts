import { vi } from "vitest";

import type { VoyaCommands } from "@voya/contracts";
import { setVoyaCommands } from "@voya/client/transport";

/**
 * Registers a fake command surface, the way an app registers its platform.
 *
 * Nothing here mocks a module: the hooks reach the backend through
 * `voyaCommands()`, so the seam is the thing to replace. Anything the test did
 * not name rejects rather than returning `undefined`, which is what turns
 * "this hook called a command I did not expect" into a readable failure
 * instead of a `TypeError` three frames away.
 */
export function installFakeCommands<Commands extends Partial<VoyaCommands>>(
  commands: Commands,
): Commands {
  const surface = new Proxy(commands, {
    get(target, property: string) {
      if (property in target) {
        return target[property as keyof Commands];
      }

      return vi.fn(() =>
        Promise.reject(new Error(`the test did not stub ${property}()`)),
      );
    },
  });

  setVoyaCommands(surface as unknown as VoyaCommands);

  return commands;
}
