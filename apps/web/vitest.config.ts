import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Pages are rendered to strings at build time; nothing runs in a browser.
    environment: "node",
  },
});
