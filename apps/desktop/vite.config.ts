import babel from "@rolldown/plugin-babel";
import tailwindcss from "@tailwindcss/vite";
import react, { reactCompilerPreset } from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";
import { configDefaults, defineConfig, lazyPlugins } from "vite-plus";

type ModuleGraph = {
  getModuleInfo(id: string): { importers: readonly string[]; isEntry: boolean } | null;
};

const startupModules = new Map<string, boolean>();

/**
 * Whether `id` is statically imported, directly or not, by the entry: whether
 * it must load before the first paint. Walks static importers only, so code
 * behind a dynamic `import()` is not startup code.
 */
function isStartupModule(id: string, graph: ModuleGraph): boolean {
  const known = startupModules.get(id);
  if (known !== undefined) return known;
  const seen = new Set([id]);
  const queue = [id];
  let startup = false;
  for (let index = 0; index < queue.length && !startup; index += 1) {
    const current = queue[index]!;
    const info = graph.getModuleInfo(current);
    startup = Boolean(info?.isEntry) || startupModules.get(current) === true;
    for (const importer of info?.importers ?? []) {
      if (!seen.has(importer)) {
        seen.add(importer);
        queue.push(importer);
      }
    }
  }
  startupModules.set(id, startup);
  return startup;
}

/**
 * A group that captures only startup modules. A plain `test` also swept
 * modules only lazy screens import into the chunk, and a startup chunk
 * loads with everything in it: the form library, the menus' Radix packages
 * and the flag URL table all loaded before the first paint that way. The
 * modules left out fall to a later group or to the lazy chunks that use them.
 */
function startupGroup(name: string) {
  return (id: string, graph: ModuleGraph) => (isStartupModule(id, graph) ? name : null);
}

// Intel Macs stop at systems older than Apple Silicon ones do, and macOS 12's
// WKWebView is Safari 15-level: it cannot parse the class static blocks the
// default target (Safari 16.4) keeps, so the entry module fails and the window
// stays blank. Tauri exports these two to `beforeBuildCommand`, following
// `--target` or else the host. A plain `vp build` has neither and keeps the
// default, which is what the bundle budgets measure and what every other
// platform ships. Each target is the WebKit of that package's
// `minimumSystemVersion` (scripts/tauri/macos-minimum.mjs): 15.4 is macOS 12.3,
// the first with the cascade layers Tailwind's output depends on.
const intelMac = process.env.TAURI_ENV_PLATFORM === "darwin" && process.env.TAURI_ENV_ARCH === "x86_64";
const intelMacTarget = "safari15.4";

type SyntaxNode = { type?: string; regex?: { pattern: string; flags: string } };

/** Names what `node` or anything under it uses that Safari 15.4 cannot parse. */
function syntaxNewerThanIntelMac(node: unknown, found = new Set<string>()): Set<string> {
  if (Array.isArray(node)) {
    for (const child of node) syntaxNewerThanIntelMac(child, found);
  } else if (node !== null && typeof node === "object") {
    const { type, regex } = node as SyntaxNode;
    if (type === "StaticBlock") found.add("a class static block");
    if (regex && /\(\?<[=!]/.test(regex.pattern)) found.add(`a lookbehind in /${regex.pattern}/`);
    if (regex?.flags.includes("v")) found.add(`the v flag on /${regex.pattern}/`);
    for (const child of Object.values(node)) syntaxNewerThanIntelMac(child, found);
  }
  return found;
}

/**
 * Fails an Intel Mac build whose output the oldest supported WKWebView would
 * reject. The target lowers what it can; a dependency can still ship syntax
 * nothing lowers (a regex lookbehind), and no build machine runs a WebKit old
 * enough to show the blank window that results.
 */
function intelMacSyntaxGate() {
  return {
    name: "voya:intel-mac-syntax",
    generateBundle(
      this: { parse(code: string): unknown; error(message: string): never },
      _options: unknown,
      bundle: Record<string, { type: string; code?: string }>,
    ) {
      for (const [fileName, chunk] of Object.entries(bundle)) {
        if (chunk.type !== "chunk" || chunk.code === undefined) continue;
        const found = [...syntaxNewerThanIntelMac(this.parse(chunk.code))];
        if (found.length > 0) {
          this.error(`${fileName} uses ${found.join(", ")}, which ${intelMacTarget} cannot parse.`);
        }
      }
    },
  };
}

export default defineConfig({
  clearScreen: false,
  // React Compiler memoizes components and hooks, so a screen fed by a
  // per-frame store update (speedtest results, connections, logs) re-renders
  // only what changed. The `useVirtualizer` call sites opt out; see their
  // `react-hooks/incompatible-library` notes. Unit tests run the source as
  // written: Babel's on-demand transform pushed lazy screens past Testing
  // Library's timeouts, and several suites drive non-reactive store mocks that
  // rely on whole-tree re-renders. The renderer smoke runs compiled code, and
  // Oxlint enforces the compiler's rules on the source.
  plugins: lazyPlugins(() => [
    react(),
    ...(process.env.VITEST ? [] : [babel({ presets: [reactCompilerPreset()] })]),
    tailwindcss(),
    ...(intelMac ? [intelMacSyntaxGate()] : []),
  ]),
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    // Playwright traces contain HTML; writing diagnostics must never reload the app.
    watch: { ignored: ["**/.agents/**", "**/test-results/**", "**/playwright-report/**", "**/*-snapshots/**"] },
  },
  build: {
    ...(intelMac ? { target: intelMacTarget } : {}),
    // Flags stay separate files: inlined, all ~250 would ride in the chunk that
    // imports them (node-country-icon.tsx) although a screen shows a handful.
    assetsInlineLimit: (filePath) => (/[\\/]flag-icons[\\/]flags[\\/]/.test(filePath) ? false : undefined),
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            {
              name: "vendor-react",
              priority: 40,
              test: /node_modules[\\/](react|react-dom)[\\/]/,
            },
            {
              // What the shell itself renders: the toast primitives. Dialogs
              // and the checkbox load with whatever opens them.
              name: startupGroup("vendor-radix"),
              debugName: "vendor-radix",
              priority: 34,
              test: /node_modules[\\/]@radix-ui[\\/]/,
            },
            {
              // Select, menus, tabs, tooltips and the floating-ui positioning
              // under them: only screens use them, and several do.
              name: "vendor-menus",
              priority: 32,
              test: /node_modules[\\/](@radix-ui|@floating-ui)[\\/]/,
            },
            {
              // Drag-and-drop only serves the Rules page; keeping it out of the
              // generic vendor chunk keeps it off the startup path.
              name: "vendor-dnd",
              priority: 35,
              test: /node_modules[\\/]@dnd-kit[\\/]/,
            },
            {
              name: startupGroup("vendor-icons"),
              debugName: "vendor-icons",
              priority: 33,
              test: /node_modules[\\/]lucide-react[\\/]/,
            },
            {
              // English, the fallback locale, is data rather than application
              // code. Splitting it keeps the entry budget a guard on *code*
              // growth instead of a cap that ordinary translation work walks
              // into. It loads at startup; the other locales stay out of this
              // group so each remains its own on-demand chunk.
              name: "locales",
              priority: 30,
              test: /packages[\\/]i18n[\\/]src[\\/]locales[\\/]en\.json$/,
            },
            {
              name: startupGroup("vendor-data"),
              debugName: "vendor-data",
              priority: 20,
              test: /node_modules[\\/](@tanstack|i18next|zustand)[\\/]/,
            },
            {
              // Only the editing dialogs and settings validate forms.
              name: "vendor-forms",
              priority: 20,
              test: /node_modules[\\/](zod)[\\/]/,
            },
            {
              name: startupGroup("vendor"),
              debugName: "vendor",
              priority: 10,
              test: /node_modules[\\/]/,
            },
          ],
        },
      },
    },
  },
  test: {
    environment: "jsdom",
    exclude: [...configDefaults.exclude, "e2e/**"],
    globals: true,
    // Worker threads instead of the default child processes; each file still
    // gets a fresh environment. Measured 2026-09-19: 13.7-15.0 s -> 11.7 s.
    pool: "threads",
    setupFiles: "./src/test/setup.ts",
    // Heavy interaction tests (e.g. the protocol-dialog walkthrough) can exceed
    // the 5s default under parallel CPU contention; give them comfortable margin
    // while still catching genuinely hung tests.
    testTimeout: 20000,
  },
});
