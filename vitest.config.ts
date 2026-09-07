import { defineConfig } from "vitest/config";

import { globalMinimums } from "./scripts/quality/frontend-coverage-policy.mjs";

export default defineConfig({
  test: {
    // Coverage is a root-level concern in a multi-project run, and the
    // thresholds live here rather than in the `check:frontend:coverage` script
    // string so that a plain `pnpm test --coverage` enforces the same floors a
    // CI run does. Per-module floors are enforced afterwards by
    // scripts/quality/frontend-coverage.mjs from the json-summary report.
    coverage: {
      include: ["apps/desktop/src/**/*.{ts,tsx}", "packages/*/src/**/*.{ts,tsx}"],
      exclude: ["apps/desktop/src/ipc/bindings.ts"],
      reporter: ["text-summary", "json-summary"],
      thresholds: globalMinimums,
    },
    projects: [
      "apps/desktop",
      "packages/*",
      {
        test: {
          name: "scripts",
          environment: "node",
          include: ["scripts/**/*.test.mjs"],
          testTimeout: 20000,
        },
      },
    ],
  },
});
