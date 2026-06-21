import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react-swc";
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
              name: "vendor-radix",
              priority: 34,
              test: /node_modules[\\/]@radix-ui[\\/]/,
            },
            {
              name: "vendor-icons",
              priority: 33,
              test: /node_modules[\\/]lucide-react[\\/]/,
            },
            {
              name: "vendor-data",
              priority: 20,
              test: /node_modules[\\/](@hookform|@tanstack|i18next|react-hook-form|zod|zustand)[\\/]/,
            },
            {
              name: "feature-profiles",
              priority: 15,
              test: /src[\\/]features[\\/](groups|profiles|subscriptions)[\\/]/,
            },
            {
              name: "feature-ops",
              priority: 14,
              test: /src[\\/]features[\\/](backup|clash|dns|logs|options|qr|routing|updates)[\\/]/,
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
