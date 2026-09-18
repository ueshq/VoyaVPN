import babel from "@rolldown/plugin-babel";
import tailwindcss from "@tailwindcss/vite";
import react, { reactCompilerPreset } from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";
import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  clearScreen: false,
  // React Compiler memoizes components and hooks, so a screen fed by a
  // per-frame store update (speedtest results, connections, logs) re-renders
  // only what changed. The `useVirtualizer` call sites opt out; see their
  // `react-hooks/incompatible-library` notes. Unit tests run the source as
  // written: Babel's on-demand transform pushed lazy screens past Testing
  // Library's timeouts, and several suites drive non-reactive store mocks that
  // rely on whole-tree re-renders. The renderer smoke runs compiled code, and
  // `eslint-plugin-react-hooks` enforces the compiler's rules on the source.
  plugins: [
    react(),
    ...(process.env.VITEST ? [] : [babel({ presets: [reactCompilerPreset()] })]),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
  build: {
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
              name: "vendor-qr",
              priority: 35,
              test: /node_modules[\\/]@zxing[\\/]/,
            },
            {
              name: "vendor-radix",
              priority: 34,
              test: /node_modules[\\/]@radix-ui[\\/]/,
            },
            {
              // Drag-and-drop only serves the Rules page; keeping it out of the
              // generic vendor chunk keeps it off the startup path.
              name: "vendor-dnd",
              priority: 35,
              test: /node_modules[\\/]@dnd-kit[\\/]/,
            },
            {
              name: "vendor-icons",
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
              name: "vendor-data",
              priority: 20,
              test: /node_modules[\\/](@hookform|@tanstack|i18next|react-hook-form|zod|zustand)[\\/]/,
            },
            {
              name: "vendor",
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
