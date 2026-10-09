import type { KnipConfig } from "knip";

import { commands } from "./scripts/commands.mjs";
import viteConfig from "./vite.config.ts";

// Knip finds the scripts a package.json script runs, but the root scripts all
// run scripts/run.mjs, which looks its target up in scripts/commands.mjs, and
// the cached checks are tasks in vite.config.ts. Both are read here, so a
// script no command reaches is still reported.
const taskScripts = Object.values(viteConfig.run?.tasks ?? {})
  .flat()
  .flatMap((command) => (typeof command === "string" ? [...(/\bnode (\S+\.mjs)/.exec(command)?.slice(1) ?? [])] : []));
const commandScripts = [
  ...Object.values(commands).flatMap(({ steps }) =>
    steps.filter(([program]) => program === "node").map(([, [script]]) => script),
  ),
  ...taskScripts,
];
const desktopPrefix = "apps/desktop/";
// One pattern for the whole list: knip also finds a few of these on its own
// (postinstall, a workflow step that runs one directly) and reports an entry
// pattern as redundant when everything it matches is already an entry.
const rootCommandScripts = `{${commandScripts.filter((script) => !script.startsWith(desktopPrefix)).join(",")}}`;

export default {
  includeEntryExports: true,
  workspaces: {
    ".": {
      entry: [rootCommandScripts],
      // A command script is run, not imported: what it exports is for its
      // own test, or nothing.
      includeEntryExports: false,
    },
    "apps/mobile": {
      entry: ["src/**/*.test.{ts,tsx}"],
      // `global.css` is Uniwind's build input, reached from `index.js` and
      // compiled by Metro, so it belongs to the project rather than to the
      // ignore list; nothing imports *from* it.
      project: ["src/**/*.{ts,tsx}", "global.css"],
      // Babel rewrites helper calls into `@babel/runtime` imports, so no
      // source file names it.
      // Gradle resolves both packages by path from settings.gradle/build.gradle.
      ignoreDependencies: ["@babel/runtime", "@react-native/codegen", "@react-native/gradle-plugin"],
    },
    "apps/desktop": {
      entry: [
        "src/**/*.{test,spec}.{ts,tsx}",
        ...commandScripts
          .filter((script) => script.startsWith(desktopPrefix))
          .map((script) => script.slice(desktopPrefix.length)),
      ],
      project: ["src/**/*.{ts,tsx,css}", "e2e/**/*.ts"],
      // flag-icons ships only SVG files, reached through `import.meta.glob`
      // in node-country-icon.tsx, which knip cannot follow.
      ignoreDependencies: ["@tauri-apps/cli", "flag-icons"],
      ignoreBinaries: ["tauri-driver"],
      ignoreIssues: {
        "src/ipc/bindings.ts": ["duplicates", "enumMembers", "exports", "types"],
      },
    },
    "apps/probe": {
      entry: ["src/index.ts", "test/**/*.test.ts"],
      project: ["src/**/*.ts", "test/**/*.ts"],
      // Deployment only: `vp run --filter @voya/probe deploy` runs a wrangler the
      // operator installs, so it is not a workspace dependency.
      ignoreBinaries: ["wrangler"],
      // `cloudflare:sockets` is a Workers runtime module, not a package.
      ignoreDependencies: ["cloudflare"],
    },
    "apps/web": {
      // scripts/build.mjs loads src/render.tsx through Vite's module runner, and
      // vite.config.ts names src/site.css as a build input, both by path, so
      // knip cannot follow either.
      entry: ["src/render.tsx", "src/site.css", "test/**/*.test.{ts,tsx}"],
      project: ["scripts/**/*.mjs", "src/**/*.{ts,tsx,css}", "test/**/*.{ts,tsx}"],
    },
    "packages/ui": {
      project: ["src/**/*.{ts,tsx,css}"],
    },
    // Generated from bindings.ts; every DTO is part of the contract whether or
    // not a frontend reads it today.
    "packages/contracts": {
      project: ["src/**/*.ts"],
      ignoreIssues: {
        "src/generated.ts": ["duplicates", "enumMembers", "exports", "types"],
      },
    },
    "packages/client": {
      entry: ["src/*.ts"],
      project: ["src/**/*.ts"],
    },
    "packages/features": {
      entry: ["src/**/*.test.{ts,tsx}"],
      project: ["src/**/*.{ts,tsx}"],
    },
  },
} satisfies KnipConfig;
