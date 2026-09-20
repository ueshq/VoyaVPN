import { setClientStorage, setSystemColorSchemeReader } from "@voya/client/platform";
import { setVoyaCommands } from "@voya/client/transport";

import * as commands from "@/ipc/commands";

/**
 * Registers the desktop platform behind `@voya/client`.
 *
 * Imported for its side effect, first, before anything that reads a store: the
 * persisted stores are created at module scope and rehydrate as soon as their
 * module is evaluated, so registering later would let them start from the
 * in-memory fallback and then silently disagree with what is on disk.
 */
setClientStorage(window.localStorage);

setSystemColorSchemeReader(() =>
  typeof window.matchMedia === "function" && window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light",
);

// `satisfies` is the whole point of the seam: dropping or mistyping a command
// in the Tauri binding fails the build here rather than at the call site.
setVoyaCommands(commands satisfies VoyaCommandsShape);

// Declared locally so the assertion reads as one line above; `VoyaCommands`
// itself is the generated contract.
type VoyaCommandsShape = import("@voya/contracts").VoyaCommands;
