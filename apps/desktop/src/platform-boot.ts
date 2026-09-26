import { setClientStorage } from "@voya/client/platform";

import { registerDesktopBackend } from "@/ipc/register-backend";

/**
 * Registers the desktop platform behind `@voya/client`.
 *
 * Imported for its side effect, first, before anything that reads a store: the
 * persisted stores are created at module scope and rehydrate as soon as their
 * module is evaluated, so registering later would let them start from the
 * in-memory fallback and then silently disagree with what is on disk.
 */
setClientStorage(window.localStorage);

registerDesktopBackend();
