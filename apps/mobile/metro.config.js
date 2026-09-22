const path = require("node:path");

const { getDefaultConfig, mergeConfig } = require("@react-native/metro-config");
const { wrapWithReanimatedMetroConfig } = require("react-native-reanimated/metro-config");
const { withUniwindConfig } = require("uniwind/metro");

const projectRoot = __dirname;
const workspaceRoot = path.resolve(projectRoot, "../..");

/**
 * Metro configuration for a pnpm workspace.
 *
 * The three settings below are not optional here:
 *
 * - `watchFolders` — the `@voya/*` packages are source-only (their `exports`
 *   map points straight at `.ts`/`.tsx`, with no build step), so Metro has to
 *   watch and transform files outside this app's directory. The Babel preset
 *   handles TypeScript from those folders the same as local files.
 * - `nodeModulesPaths` — the two roots this app's own imports resolve against.
 *   Hierarchical lookup stays *on* (Metro's default): pnpm stores each package
 *   at a real path with its declared dependencies in a sibling `node_modules`,
 *   so walking up from a file's real location is both how those dependencies
 *   are found and what keeps resolution strict. Disabling it breaks
 *   `react-native`'s own imports, which resolve from inside the store.
 * - `unstable_enablePackageExports` — every `@voya/*` package is addressed
 *   through its `exports` map (`@voya/client/query-keys`), which Metro only
 *   honours with this on.
 * - `extraNodeModules` — Babel rewrites its helpers into
 *   `@babel/runtime/helpers/…` imports, resolved relative to the file being
 *   transformed. For a file in `packages/i18n` that lookup starts there, and
 *   those packages have no reason to depend on Babel's runtime, so the app's
 *   copy is pointed at explicitly.
 *
 * @type {import('@react-native/metro-config').MetroConfig}
 */
const config = {
  projectRoot,
  watchFolders: [workspaceRoot],
  resolver: {
    nodeModulesPaths: [
      path.resolve(projectRoot, "node_modules"),
      path.resolve(workspaceRoot, "node_modules"),
    ],
    unstable_enablePackageExports: true,
    extraNodeModules: {
      "@babel/runtime": path.resolve(projectRoot, "node_modules/@babel/runtime"),
    },
  },
};

/**
 * Tailwind comes from Uniwind, not NativeWind.
 *
 * Both compile Tailwind v4 at build time, but NativeWind v5 styles through
 * `react-native-css`, whose Metro integration is built on `@expo/metro-config`
 * — and that package now fails to load without the `expo` SDK beside it, which
 * a bare React Native app does not have. Uniwind detects a non-Expo project and
 * uses Metro's own transform worker, so the app stays bare.
 *
 * `polyfills.rem` fixes the root font size, because React Native has no
 * document to read one from, and `dtsFile` is where Uniwind writes the
 * `className` types.
 *
 * `withUniwindConfig` has to stay the outermost wrapper: it swaps in its own
 * `transformerPath` and wraps `resolveRequest`, so a wrapper applied after it
 * could replace either. Reanimated's wrapper only folds its own frames out of
 * red-box stacks.
 */
module.exports = withUniwindConfig(
  wrapWithReanimatedMetroConfig(mergeConfig(getDefaultConfig(projectRoot), config)),
  {
    // Both paths are resolved against the working directory Metro runs in,
    // which is this app's directory; an absolute path would be joined onto it.
    cssEntryFile: "global.css",
    dtsFile: "uniwind-env.d.ts",
    polyfills: { rem: 16 },
  },
);
