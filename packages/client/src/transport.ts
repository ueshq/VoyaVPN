import type { VoyaCommands } from "@voya/contracts";

/**
 * The platform seam.
 *
 * Everything else in `@voya/client` is transport-agnostic: it reaches the
 * backend through the `VoyaCommands` registered here. The desktop shell
 * registers its Tauri binding from `apps/desktop/src/ipc`; a React Native app
 * registers a native-module binding. Neither implementation leaks into shared
 * code, and `satisfies VoyaCommands` makes a missing command a compile error.
 */
let registered: VoyaCommands | null = null;

export function setVoyaCommands(commands: VoyaCommands) {
  registered = commands;
}

/**
 * The registered command surface.
 *
 * Throws rather than returning `null` because every caller is downstream of app
 * startup: a missing registration is a wiring bug, not a state a feature should
 * have to handle.
 */
export function voyaCommands(): VoyaCommands {
  if (!registered) {
    throw new Error("No VoyaCommands registered — call setVoyaCommands() during app startup.");
  }

  return registered;
}
