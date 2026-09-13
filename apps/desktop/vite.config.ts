import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";
import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  clearScreen: false,
  plugins: [react(), tailwindcss()],
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
              name: "vendor-editor",
              priority: 35,
              test: /node_modules[\\/](@codemirror|@uiw)[\\/]/,
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
              // The shipped locales are data, not application code, and
              // they are ~40% of what the entry chunk used to weigh. Splitting
              // them keeps the entry budget a guard on *code* growth — an
              // accidental dependency import — instead of a cap that ordinary
              // translation work walks into. Both chunks load at startup, and
              // `frontend-bundle.mjs` budgets each one plus the total.
              name: "locales",
              priority: 30,
              test: /packages[\\/]i18n[\\/]src[\\/]locales[\\/]/,
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
    setupFiles: "./src/test/setup.ts",
    // Heavy interaction tests (e.g. the protocol-dialog walkthrough) can exceed
    // the 5s default under parallel CPU contention; give them comfortable margin
    // while still catching genuinely hung tests.
    testTimeout: 20000,
  },
});
