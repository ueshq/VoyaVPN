/**
 * Platform facts the shared stores need but cannot observe for themselves.
 *
 * The desktop shell reads them from the DOM (`window.localStorage`,
 * `matchMedia`); React Native reads them from MMKV and `Appearance`. Injecting
 * them keeps every persisted store in this package free of a `window`
 * reference, which is the only thing that made them desktop-private before.
 */

/** The subset of the Web Storage API zustand's `createJSONStorage` needs. */
export type ClientStorage = {
  getItem: (name: string) => string | null | Promise<string | null>;
  setItem: (name: string, value: string) => unknown;
  removeItem: (name: string) => unknown;
};

/**
 * An in-memory fallback.
 *
 * A store may be imported before startup wiring runs (module-level `create()`
 * calls do that), and tests rarely care about persistence. Failing there would
 * turn a wiring detail into a crash, so the default forgets instead.
 */
function createMemoryStorage(): ClientStorage {
  const entries = new Map<string, string>();

  return {
    getItem: (name) => entries.get(name) ?? null,
    setItem: (name, value) => entries.set(name, value),
    removeItem: (name) => entries.delete(name),
  };
}

let storage: ClientStorage = createMemoryStorage();

export function setClientStorage(next: ClientStorage) {
  storage = next;
}

/**
 * A stable handle that forwards to whatever is registered *at call time*.
 *
 * zustand's `createJSONStorage(getStorage)` invokes `getStorage` once, while
 * the store module is being evaluated — so returning the current storage
 * directly would let a store capture the in-memory fallback forever if its
 * module happened to load before startup wiring. Every module-order hazard
 * disappears by handing out this delegate instead.
 */
const delegate: ClientStorage = {
  getItem: (name) => storage.getItem(name),
  setItem: (name, value) => storage.setItem(name, value),
  removeItem: (name) => storage.removeItem(name),
};

export function clientStorage(): ClientStorage {
  return delegate;
}

export type ColorScheme = "dark" | "light";

let readColorScheme: () => ColorScheme = () => "light";

export function setSystemColorSchemeReader(read: () => ColorScheme) {
  readColorScheme = read;
}

/** The OS-level light/dark preference, for resolving the `system` theme mode. */
export function systemColorScheme(): ColorScheme {
  return readColorScheme();
}
