import { createMMKV } from "react-native-mmkv";

/**
 * The device key/value store.
 *
 * MMKV rather than AsyncStorage because zustand's `persist` rehydrates
 * synchronously when the storage is synchronous: an async store makes every
 * persisted slice start from its default and then swap, which is a visible
 * flash on the tabs that remember a filter or a collapsed group.
 */
export const storage = createMMKV({ id: "voyavpn" });

/** The shape `@voya/client` persists through, backed by MMKV. */
export const clientStorageAdapter = {
  getItem: (name: string) => storage.getString(name) ?? null,
  setItem: (name: string, value: string) => {
    storage.set(name, value);
  },
  removeItem: (name: string) => {
    storage.remove(name);
  },
};
