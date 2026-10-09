import { isCliEntrypoint, runOrExit } from "../lib/common.mjs";

/**
 * The canonical gate list. AGENTS.md and README.md mirror this array, the CI
 * `baseline-*` jobs split it between them and together run each gate once, and
 * scripts/quality/verify-local.test.mjs fails when any of them drifts from it.
 * `vp run` executes the package scripts, so it has to be on PATH: the global
 * Vite+ CLI locally, `voidzero-dev/setup-vp` in CI.
 */
export const steps = [
  ["Architecture boundaries", "vp", ["run", "check", "architecture"]],
  ["Lockfile single versions", "vp", ["run", "check", "lockfile"]],
  ["Rust formatting", "vp", ["run", "check", "rust", "fmt"]],
  ["Rust Clippy", "vp", ["run", "check", "rust", "clippy"]],
  ["Rust dependency usage", "vp", ["run", "check", "rust", "deps"]],
  ["Rust tests", "vp", ["run", "check", "rust", "test"]],
  ["Frontend format, lint and typecheck", "vp", ["check"]],
  ["Frontend tests and coverage", "vp", ["run", "check", "frontend", "coverage"]],
  ["Frontend production bundle", "vp", ["run", "check", "frontend", "bundle"]],
  ["Frontend mock smoke tests", "vp", ["run", "check", "frontend", "smoke", "mock"]],
  ["Dead code and dependency usage", "vp", ["run", "check", "dead-code"]],
  ["sing-box config acceptance", "vp", ["run", "check", "sing-box"]],
  ["Generated binding drift", "vp", ["run", "check", "bindings"]],
  ["i18n locale drift", "vp", ["run", "check", "i18n"]],
];

if (isCliEntrypoint(import.meta.url)) {
  for (const [name, command, args] of steps) {
    console.log(`\n==> ${name}`);
    console.log(`$ ${[command, ...args].join(" ")}`);
    runOrExit(command, args, {
      env: { ...process.env, CI: process.env.CI ?? "true" },
      // Printed above, as the command a reader would type.
      log: false,
    });
  }

  console.log("\nLocal verification checks passed.");
}
