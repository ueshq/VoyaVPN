import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import tseslint from "typescript-eslint";

const tauriBoundaryMessage = "ADR-0002: only apps/desktop/src/ipc may import Tauri APIs.";
const tauriGlobalsMessage = "ADR-0002: only apps/desktop/src/ipc may touch the Tauri runtime globals.";

// no-restricted-imports only inspects static import/export declarations, so
// dynamic import() and require() of Tauri APIs need their own selectors.
const tauriImportSelectors = [
  { selector: "ImportExpression > Literal[value=/^@tauri-apps/]", message: tauriBoundaryMessage },
  {
    selector: "CallExpression[callee.name='require'] > Literal[value=/^@tauri-apps/]",
    message: tauriBoundaryMessage,
  },
];

// Reaching window.__TAURI_INTERNALS__ bypasses the typed wrappers just like a
// direct import does. A `"__TAURI_INTERNALS__" in window` presence probe is
// still allowed, and tests may install the globals to simulate the shell.
const tauriGlobalSelectors = [
  { selector: "MemberExpression[property.name=/^__TAURI/]", message: tauriGlobalsMessage },
  { selector: "MemberExpression[computed=true] > Literal[value=/^__TAURI/]", message: tauriGlobalsMessage },
];

export default tseslint.config(
  { ignores: ["**/dist", "**/node_modules", "target", "apps/desktop/src-tauri/gen"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: 2022,
      globals: globals.browser,
      parserOptions: {
        projectService: {
          allowDefaultProject: ["vitest.config.ts"],
        },
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
    },
  },
  {
    files: [
      "apps/desktop/vite.config.ts",
      "apps/desktop/playwright.config.ts",
      "**/vitest.config.ts",
      "eslint.config.js",
      "scripts/**/*.mjs",
      "apps/desktop/e2e/**/*.ts",
    ],
    languageOptions: {
      globals: { ...globals.browser, ...globals.node },
    },
  },
  {
    // The type-aware tier. `projectService` above already builds the full
    // TypeScript program for every .ts/.tsx file, so these rules cost nothing
    // extra — and they are the only check for the failure mode an IPC-heavy app
    // actually has: a dropped `commands.*()` / `mutateAsync()` promise, or an
    // async handler passed straight to `onClick`/`onSubmit`. The ~34
    // hand-written `void somePromise()` statements in apps/desktop/src show the
    // team was already policing this by hand.
    //
    // `no-unnecessary-condition`, `require-await`, `no-unnecessary-type-assertion`
    // and `consistent-type-imports` are deliberately *not* enabled: measured on
    // this tree they report 100 / 25 / 18 / 5 findings, which is a codemod, not
    // a gate. `switch-exhaustiveness-check` runs with
    // `considerDefaultExhaustiveForUnions` so a switch that already has a
    // `default:` arm is accepted, while a bare switch over an `AppError` or
    // event union must list every member.
    files: ["apps/*/src/**/*.{ts,tsx}", "packages/**/*.{ts,tsx}", "apps/desktop/e2e/**/*.ts"],
    rules: {
      "@typescript-eslint/await-thenable": "error",
      "@typescript-eslint/no-floating-promises": "error",
      "@typescript-eslint/no-misused-promises": "error",
      "@typescript-eslint/switch-exhaustiveness-check": [
        "error",
        { considerDefaultExhaustiveForUnions: true },
      ],
    },
  },
  {
    files: ["apps/desktop/src/ipc/bindings.ts"],
    rules: {
      "@typescript-eslint/no-explicit-any": "off",
    },
  },
  {
    files: ["apps/*/src/**/*.{ts,tsx}", "packages/**/*.{ts,tsx}"],
    ignores: ["apps/desktop/src/ipc/**"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["@tauri-apps/api", "@tauri-apps/api/*", "@tauri-apps/plugin-*"],
              message: tauriBoundaryMessage,
            },
          ],
        },
      ],
      "no-restricted-syntax": ["error", ...tauriImportSelectors],
    },
  },
  {
    files: ["apps/*/src/**/*.{ts,tsx}", "packages/**/*.{ts,tsx}"],
    ignores: [
      "apps/desktop/src/ipc/**",
      "**/*.{test,spec}.{ts,tsx}",
      "**/test/**",
      "**/*.test-fixture.ts",
    ],
    rules: {
      "no-restricted-syntax": ["error", ...tauriImportSelectors, ...tauriGlobalSelectors],
    },
  },
);
