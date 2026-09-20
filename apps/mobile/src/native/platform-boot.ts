import { setClientStorage, setSystemColorSchemeReader } from "@voya/client/platform";
import { createNativeI18n } from "@voya/i18n/native";
import { Appearance } from "react-native";

import { deviceLanguages } from "./locale";
import { clientStorageAdapter, storage } from "./storage";

/**
 * Registers the React Native platform behind the shared packages.
 *
 * Imported for its side effect, first, before anything that reads a store: the
 * persisted stores are created at module scope and rehydrate as soon as their
 * module is evaluated, so registering later would let them start from the
 * in-memory fallback and then silently disagree with what is on disk.
 *
 * The desktop equivalent is `apps/desktop/src/platform-boot.ts`. The command
 * surface is not registered here yet — that arrives with the native module in a
 * later phase; until then nothing in this app calls a backend command.
 */
setClientStorage(clientStorageAdapter);

setSystemColorSchemeReader(() => (Appearance.getColorScheme() === "dark" ? "dark" : "light"));

// MMKV's `getString`/`set` are already the shape the i18n host asks for.
const i18n = createNativeI18n({ storage, deviceLanguages });

/**
 * Settles once the startup locale's resources are in place, and never rejects.
 *
 * A stable, already-caught promise because the shell suspends on it with
 * `use()`: a new promise per render would suspend forever, and a rejection
 * would surface as a render error rather than the English fallback the failure
 * actually leaves behind.
 */
export const localeReady: Promise<void> = i18n.localeReady.catch(() => {});
