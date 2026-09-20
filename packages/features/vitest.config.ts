import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // The hooks here are DOM-free, but React needs a renderer to run them and
    // Testing Library's is the cheap one; React Native screens exercise the
    // same hooks through RNTL in `apps/mobile`.
    environment: "jsdom",
    globals: true,
    setupFiles: "./src/test/setup.ts",
  },
});
