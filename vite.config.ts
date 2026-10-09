import { defineConfig } from "vite-plus";

import { globalMinimums } from "./scripts/quality/frontend-coverage-policy.mjs";

// The ESLint flat config this replaces is in git history (eslint.config.js).
// Oxlint reads `eslint-disable` comments too, but new ones use `oxlint-disable`.
const tauriBoundaryMessage = "ADR-0002: only apps/desktop/src/ipc may import Tauri APIs.";
const sharedPlatformMessage = "Shared frontend code must reach the platform through @voya/client/platform.";
const hermesMessage = "Hermes does not implement every ES2023 copying method. Copy the array, then mutate the copy.";

export default defineConfig({
  // Bare `vp dev` / `vp build` / `vp preview` at the root mean the desktop
  // renderer: every package has a vite.config.ts for its tests, so without
  // this vp would ask which one. Read statically, so it must stay a literal.
  defaultPackage: "./apps/desktop",
  // Commands are the words after `vp run` (`vp run build mac local`), resolved
  // by scripts/commands.mjs. Only the checks below are tasks, because a task
  // is cached: it replays its last result while the files it read are
  // unchanged, and runs in a clean environment. That suits a check that reads
  // tracked sources and nothing else; a build, a signing step or anything
  // driven by cargo stays an uncached command. `cachedTasks` in
  // scripts/commands.mjs maps each one to its command (`vp run check i18n`).
  run: {
    tasks: {
      "check-architecture": "node scripts/quality/architecture.mjs",
      "check-lockfile": "node scripts/quality/lockfile-duplicates.mjs",
      "check-i18n": "node scripts/quality/i18n.mjs",
    },
  },
  // The pre-commit hook (.vite-hooks/pre-commit) runs `vp staged`. The globs
  // are the file types Oxfmt formats here: Markdown and TOML are left alone.
  staged: {
    "*.{ts,tsx,js,mjs,cjs,json,css}": "vp check --fix",
  },
  fmt: {
    // The code predates the formatter and mixes line widths; 120 is the width
    // that rewrites the fewest lines.
    printWidth: 120,
    sortPackageJson: false,
    ignorePatterns: [
      // Generated, and compared byte for byte by check bindings.
      "apps/desktop/src/ipc/bindings.ts",
      "packages/contracts/src/generated.ts",
      "packages/contracts/src/commands.ts",
      "packages/contracts/*.json",
      // Rewritten by Uniwind on every Metro bundle.
      "apps/mobile/uniwind-env.d.ts",
      // Fixtures shared with Rust and patches applied by pnpm, kept as written.
      "tests/**",
      "patches/**",
      "pnpm-lock.yaml",
      // Native projects, bundled resources and the Rust side have their own tools.
      "**/resources/**",
      "apps/mobile/ios/**",
      "apps/mobile/android/**",
      "apps/desktop/src-tauri/**",
      "crates/**",
      "**/*.toml",
      // Prose: Oxfmt would pad every Markdown table to its widest cell.
      "**/*.md",
      "**/dist/**",
      "coverage/**",
      "target/**",
      ".agents/**",
    ],
  },
  lint: {
    plugins: ["eslint", "typescript", "react", "oxc"],
    jsPlugins: [{ name: "vite-plus", specifier: "vite-plus/oxlint-plugin" }, "./scripts/lint/voya-plugin.mjs"],
    categories: { correctness: "error" },
    options: {
      // typeAware runs the typescript-eslint rules that need the program;
      // typeCheck reports TypeScript's own diagnostics in the same pass, which
      // is the workspace's only type check (`vp check`).
      typeAware: true,
      typeCheck: true,
    },
    env: { builtin: true, es2024: true },
    ignorePatterns: [
      "**/dist",
      "**/node_modules",
      "**/.wrangler",
      ".agents/**",
      "target",
      "coverage",
      "apps/desktop/src-tauri/gen",
      "apps/mobile/android/**/build/**",
      "apps/mobile/ios/build/**",
    ],
    rules: {
      // What `js.configs.recommended` and `tseslint.configs.recommended`
      // enabled beyond Oxlint's correctness category.
      "no-case-declarations": "error",
      "no-empty": "error",
      "no-fallthrough": "error",
      "no-prototype-builtins": "error",
      "no-redeclare": "error",
      "no-regex-spaces": "error",
      "no-unassigned-vars": "error",
      "no-unused-vars": "error",
      "no-useless-catch": "error",
      "no-useless-escape": "error",
      "preserve-caught-error": "error",
      "no-array-constructor": "error",
      "no-unused-expressions": "error",
      "typescript/ban-ts-comment": "error",
      "typescript/no-empty-object-type": "error",
      "typescript/no-explicit-any": "error",
      "typescript/no-namespace": "error",
      "typescript/no-require-imports": "error",
      "typescript/no-unnecessary-type-constraint": "error",
      "typescript/no-unsafe-declaration-merging": "error",
      "typescript/no-unsafe-function-type": "error",
      "typescript/no-wrapper-object-types": "error",
      "typescript/prefer-as-const": "error",
      "typescript/prefer-namespace-keyword": "error",
      "typescript/triple-slash-reference": "error",
      "vite-plus/prefer-vite-plus-imports": "error",
      // The correctness category switches on every type-aware rule. Only the
      // four the override below names were ever gated here; the others are
      // left for a deliberate decision rather than adopted by the migration.
      "typescript/await-thenable": "off",
      "typescript/no-base-to-string": "off",
      "typescript/no-misused-spread": "off",
      "typescript/require-array-sort-compare": "off",
      "typescript/restrict-template-expressions": "off",
      "typescript/unbound-method": "off",
    },
    overrides: [
      {
        // TypeScript already reports what these catch, and `no-undef` cannot
        // see type-only names (tseslint's eslint-recommended override).
        files: ["**/*.{ts,tsx,mts,cts}"],
        rules: {
          "no-undef": "off",
          "no-redeclare": "off",
          "no-var": "error",
          "prefer-const": "error",
          "prefer-rest-params": "error",
          "prefer-spread": "error",
        },
      },
      {
        // eslint-plugin-react-hooks v7 `recommended`, which Oxlint implements
        // natively, React Compiler diagnostics included. Its `config` and
        // `gating` rules validate compiler options Oxlint does not expose.
        files: ["**/*.{ts,tsx}"],
        env: { browser: true },
        rules: {
          "react/rules-of-hooks": "error",
          "react/exhaustive-deps": "error",
          "react/static-components": "error",
          "react/use-memo": "error",
          "react/preserve-manual-memoization": "error",
          "react/incompatible-library": "error",
          "react/immutability": "error",
          "react/globals": "error",
          "react/refs": "error",
          "react/set-state-in-effect": "error",
          "react/error-boundaries": "error",
          "react/purity": "error",
          "react/set-state-in-render": "error",
          "react/unsupported-syntax": "error",
          "react/only-export-components": ["error", { allowConstantExport: true }],
        },
      },
      {
        // Node only: a stray `window` or `document` in a build script is a bug
        // `no-undef` should name.
        files: ["scripts/**/*.mjs", "apps/web/scripts/**/*.mjs", "apps/desktop/*.mjs", "apps/mobile/*.js"],
        env: { node: true },
      },
      {
        // The tauri-driver smoke runs in Node and hands callbacks to the page.
        files: ["apps/desktop/e2e/**/*.{ts,mjs}"],
        env: { browser: true, node: true },
      },
      {
        // React Native tooling: Metro and Babel load their configs with
        // `require`, so CommonJS is the required format here, not a style choice.
        files: ["apps/mobile/*.js"],
        rules: { "typescript/no-require-imports": "off" },
      },
      {
        // Jest's module mocks are factories that `require` inside `jest.mock`,
        // which hoisting runs before any `import`; and the test helpers export
        // components alongside functions, which fast refresh never sees.
        files: ["apps/mobile/src/test/**"],
        env: { jest: true, node: true },
        rules: {
          "typescript/no-require-imports": "off",
          "react/only-export-components": "off",
        },
      },
      {
        // Plain browser scripts Vite copies as-is, such as the pre-render theme boot.
        files: ["apps/desktop/public/**/*.js"],
        env: { browser: true },
      },
      {
        // The shared frontend layer runs in the desktop WebView and in React
        // Native alike, so reaching the DOM has to fail lint here rather than on
        // a device. TypeScript cannot catch it: its `lib` includes DOM for the
        // whole workspace, and the jsdom-backed tests in these packages need it.
        // `@voya/ui` is deliberately not listed — its primitives are Radix, and
        // Radix is the DOM.
        files: ["packages/client/src/**/*.{ts,tsx}", "packages/features/src/**/*.{ts,tsx}"],
        rules: {
          "no-restricted-globals": [
            "error",
            { name: "document", message: sharedPlatformMessage },
            { name: "navigator", message: sharedPlatformMessage },
            { name: "window", message: sharedPlatformMessage },
            {
              // Every other runtime has it, which is what makes it dangerous: it
              // typechecks, it passes the jsdom tests, and then it is a fatal
              // ReferenceError the first time a release build mounts the screen.
              name: "structuredClone",
              message: "React Native does not define structuredClone. Use a local JSON clone for IPC DTOs.",
            },
          ],
        },
      },
      {
        // The DOM test harness these packages share.
        files: ["packages/features/src/test/**"],
        rules: { "no-restricted-globals": "off" },
      },
      {
        // Everything Metro bundles runs on Hermes, which lags the ES2023 `lib`
        // the workspace typechecks against. Jest runs on Node, so a missing
        // method passes every test and throws `undefined is not a function` on
        // a device.
        files: ["apps/mobile/src/**/*.{ts,tsx}", "packages/{client,contracts,features,i18n,utils}/src/**/*.{ts,tsx}"],
        rules: {
          "no-restricted-properties": [
            "error",
            { property: "toSorted", message: hermesMessage },
            { property: "toReversed", message: hermesMessage },
            { property: "toSpliced", message: hermesMessage },
          ],
        },
      },
      {
        // The type-aware tier: the only check for the failure mode an IPC-heavy
        // app actually has, a dropped `commands.*()` / `mutateAsync()` promise
        // or an async handler passed straight to `onClick`/`onSubmit`.
        //
        // `no-unnecessary-condition`, `require-await`,
        // `no-unnecessary-type-assertion` and `consistent-type-imports` are
        // deliberately *not* enabled: measured on this tree they report
        // 100 / 25 / 18 / 5 findings, which is a codemod, not a gate.
        // `switch-exhaustiveness-check` runs with
        // `considerDefaultExhaustiveForUnions` so a switch that already has a
        // `default:` arm is accepted, while a bare switch over an `AppError` or
        // event union must list every member.
        files: ["apps/*/src/**/*.{ts,tsx}", "packages/**/*.{ts,tsx}", "apps/desktop/e2e/**/*.ts"],
        rules: {
          "typescript/await-thenable": "error",
          "typescript/no-floating-promises": "error",
          "typescript/no-misused-promises": "error",
          "typescript/switch-exhaustiveness-check": ["error", { considerDefaultExhaustiveForUnions: true }],
        },
      },
      {
        files: ["apps/desktop/src/ipc/bindings.ts"],
        rules: { "typescript/no-explicit-any": "off" },
      },
      {
        // ADR-0002. `no-restricted-imports` sees static imports; the voya
        // plugin covers `import()`, `require()` and the `__TAURI*` globals.
        files: ["apps/*/src/**/*.{ts,tsx}", "packages/**/*.{ts,tsx}"],
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
          "voya/no-tauri-dynamic-import": "error",
          "voya/no-tauri-globals": "error",
        },
      },
      {
        // Tests may install the Tauri globals to simulate the shell.
        files: ["**/*.{test,spec}.{ts,tsx}", "**/test/**", "**/*.test-fixture.ts"],
        rules: { "voya/no-tauri-globals": "off" },
      },
      {
        files: ["apps/desktop/src/ipc/**"],
        rules: {
          "no-restricted-imports": "off",
          "voya/no-tauri-dynamic-import": "off",
          "voya/no-tauri-globals": "off",
        },
      },
    ],
  },
  test: {
    // Coverage instruments every workspace project. Limit concurrent workers
    // so jsdom renders and their async assertions are not starved by a full
    // machine-wide worker pool on high-core development hosts.
    maxWorkers: 4,
    // Coverage is a root-level concern in a multi-project run, and the
    // thresholds live here rather than in the `check frontend coverage` script
    // string so that a plain `vp test --coverage` enforces the same floors a
    // CI run does. Per-module floors are enforced afterwards by
    // scripts/quality/frontend-coverage.mjs from the json-summary report.
    coverage: {
      include: [
        "apps/desktop/src/**/*.{ts,tsx}",
        "apps/probe/src/**/*.ts",
        "apps/web/src/**/*.{ts,tsx}",
        "packages/*/src/**/*.{ts,tsx}",
      ],
      exclude: [
        "apps/desktop/src/ipc/bindings.ts",
        // The Worker entry only binds `cloudflare:sockets`, which exists in the
        // Workers runtime alone; the handler it calls is covered in probe.ts.
        "apps/probe/src/index.ts",
      ],
      reporter: ["text-summary", "json-summary"],
      thresholds: globalMinimums,
    },
    projects: [
      "apps/desktop",
      // The self-hosted node's reachability probe (a Cloudflare Worker).
      "apps/probe",
      // The marketing site, rendered to static HTML at build time.
      "apps/web",
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
