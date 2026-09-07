import { spawnSync } from "node:child_process";

import { isCliEntrypoint } from "../lib/common.mjs";

/**
 * The canonical gate list. AGENTS.md, README.md and the CI `baseline` job all
 * mirror this array, and scripts/quality/verify-local.test.mjs fails when any of
 * them drifts from it.
 */
export const steps = [
  ["Architecture boundaries", "pnpm", ["run", "check:architecture"]],
  ["Lockfile single versions", "pnpm", ["run", "check:lockfile"]],
  ["Rust formatting", "pnpm", ["run", "check:rust:fmt"]],
  ["Rust Clippy", "pnpm", ["run", "check:rust:clippy"]],
  ["Rust dependency usage", "pnpm", ["run", "check:rust:deps"]],
  ["Rust tests", "pnpm", ["run", "check:rust:test"]],
  ["Frontend typecheck", "pnpm", ["run", "check:frontend:typecheck"]],
  ["Frontend tests and coverage", "pnpm", ["run", "check:frontend:coverage"]],
  ["Frontend lint", "pnpm", ["run", "check:frontend:lint"]],
  ["Frontend production bundle", "pnpm", ["run", "check:frontend:bundle"]],
  ["Frontend mock smoke tests", "pnpm", ["run", "check:frontend:smoke:mock"]],
  ["Dead code and dependency usage", "pnpm", ["run", "check:dead-code"]],
  ["sing-box config acceptance", "pnpm", ["run", "check:sing-box"]],
  ["Generated binding drift", "pnpm", ["run", "check:bindings"]],
  ["i18n locale drift", "pnpm", ["run", "check:i18n"]],
];

if (isCliEntrypoint(import.meta.url)) {
  for (const [name, command, args] of steps) {
    console.log(`\n==> ${name}`);
    console.log(`$ ${[command, ...args].join(" ")}`);
    const invocation = executable(command, args);

    const result = spawnSync(invocation.file, invocation.args, {
      env: { ...process.env, CI: process.env.CI ?? "true" },
      stdio: "inherit",
    });

    if (result.status !== 0) {
      process.exit(result.status ?? 1);
    }
  }

  console.log("\nLocal verification checks passed.");
}

function executable(command, args) {
  if (command === "pnpm" && process.env.npm_execpath) {
    return { file: process.execPath, args: [process.env.npm_execpath, ...args] };
  }

  return { file: command, args };
}
