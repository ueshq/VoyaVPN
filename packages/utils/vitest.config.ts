import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Pure helpers need no DOM. The two React hook tests opt back into jsdom
    // with a per-file `@vitest-environment` docblock, so the rest of the
    // package skips jsdom's per-file setup.
    environment: "node",
  },
});
