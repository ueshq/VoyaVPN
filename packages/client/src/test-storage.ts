import { setClientStorage, type ClientStorage } from "./platform";

/**
 * A storage seam a test can read back.
 *
 * The persisted stores used to write straight to `window.localStorage`, so
 * their tests needed jsdom. Registering this instead keeps them in the node
 * environment and makes what was written inspectable without parsing a global.
 */
export function installTestStorage() {
  const entries = new Map<string, string>();

  const storage: ClientStorage = {
    getItem: (name) => entries.get(name) ?? null,
    setItem: (name, value) => entries.set(name, value),
    removeItem: (name) => entries.delete(name),
  };

  setClientStorage(storage);

  return {
    clear: () => entries.clear(),
    read: (name: string) => entries.get(name) ?? null,
    write: (name: string, value: string) => entries.set(name, value),
  };
}
