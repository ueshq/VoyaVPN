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
  test: {
    environment: "jsdom",
    exclude: [...configDefaults.exclude, "e2e/**"],
    globals: true,
    setupFiles: "./src/test/setup.ts",
  },
});
