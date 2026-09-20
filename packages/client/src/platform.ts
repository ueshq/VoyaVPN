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

/**
 * Asks the OS for whatever authorization the tunnel needs, resolving to
 * whether it was granted.
 *
 * The prompt is platform-shaped — a native elevation dialog on desktop, the
 * VPN-configuration consent sheet on iOS and Android — but what the shared
 * retry in `runWithElevation` needs is the same everywhere: one attempt, and an
 * answer. The default declines, so an app that never registers one simply never
 * retries instead of calling a command it does not have.
 */
export type ElevationHandler = () => Promise<boolean>;

let requestElevationHandler: ElevationHandler = async () => false;

export function setElevationHandler(handler: ElevationHandler) {
  requestElevationHandler = handler;
}

export function requestElevation(): Promise<boolean> {
  return requestElevationHandler();
}

/**
 * The system clipboard.
 *
 * Both halves are platform-shaped for different reasons: the desktop writes
 * through the WebView's `navigator.clipboard` but *reads* through the backend,
 * because a WebView read needs a user gesture the paste menu item does not
 * always carry; React Native has one native module for both. Sharing the seam
 * rather than the implementation keeps `use-node-import` identical on both.
 */
export type ClipboardAdapter = {
  readText: () => Promise<string>;
  writeText: (text: string) => Promise<void>;
};

const unregisteredClipboard = (): never => {
  throw new Error("No clipboard registered — call setClipboard() during app startup.");
};

let clipboardAdapter: ClipboardAdapter = {
  readText: unregisteredClipboard,
  writeText: unregisteredClipboard,
};

export function setClipboard(adapter: ClipboardAdapter) {
  clipboardAdapter = adapter;
}

export function clipboard(): ClipboardAdapter {
  return clipboardAdapter;
}

/**
 * Whether the app is on screen.
 *
 * `false` while the desktop window is hidden into the tray or minimized, and
 * while a mobile app is backgrounded — which is when live streams (logs,
 * connection polling) should stop feeding a surface nobody is looking at.
 */
export type AppVisibilityAdapter = {
  subscribe: (onChange: () => void) => () => void;
  isVisible: () => boolean;
};

// Always visible until an app says otherwise: a missing registration must not
// silently switch every live stream off.
let appVisibility: AppVisibilityAdapter = {
  subscribe: () => () => {},
  isVisible: () => true,
};

export function setAppVisibility(adapter: AppVisibilityAdapter) {
  appVisibility = adapter;
}

export function appVisibilityAdapter(): AppVisibilityAdapter {
  return appVisibility;
}
