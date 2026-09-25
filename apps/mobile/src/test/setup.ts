// @testing-library/react-native v14 registers its matchers itself; there is
// nothing to import here for them.



// SafeAreaProvider measures the real window before it renders children, so in a
// test it renders nothing at all. The package ships this mock for exactly that.
// The shipped mock puts every component on `default`, so it has to be spread
// back out to stand in for the module's named exports.
jest.mock("react-native-safe-area-context", () => ({
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  ...require("react-native-safe-area-context/jest/mock").default,
}));

// The clipboard is a TurboModule, and `getEnforcing` throws on import when the
// native binary is not there. The seam in `@voya/client/platform` is what app
// code actually reaches, so a test that cares replaces that instead; this only
// has to keep the import from exploding.
jest.mock("@react-native-clipboard/clipboard", () => ({
  __esModule: true,
  default: { getString: async () => "", setString: () => {} },
}));

// MMKV is a JSI module with no JavaScript fallback, so a Jest run has to stand
// one in. An in-memory map matches the real synchronous semantics, which is
// what the persisted stores depend on.
jest.mock("react-native-mmkv", () => {
  class FakeMMKV {
    private readonly entries = new Map<string, string>();

    getString(key: string) {
      return this.entries.get(key);
    }

    set(key: string, value: string) {
      this.entries.set(key, value);
    }

    remove(key: string) {
      return this.entries.delete(key);
    }
  }

  return { createMMKV: () => new FakeMMKV() };
});

// Worklets owns the JSI runtime Reanimated 4 compiles worklets against. A Jest
// run has neither, and importing the real module throws on that. The shipped
// mock replaces the runtime with ordinary function calls.
// eslint-disable-next-line @typescript-eslint/no-require-imports
jest.mock("react-native-worklets", () => require("react-native-worklets/lib/module/mock"));

// The shipped Reanimated mock still requires the real package, whose import-time
// initializer calls `setCSSEventHandler` on the JS fallback and throws. The
// local stub covers the public surface HeroUI reads.
// eslint-disable-next-line @typescript-eslint/no-require-imports
jest.mock("react-native-reanimated", () => require("~/test/reanimated-mock"));

