import { defineConfig } from "vite-plus";

export default defineConfig({
  test: {
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts",
  },
});
