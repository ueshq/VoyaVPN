import { spawnSync } from "node:child_process";

const steps = [
  ["Rust formatting", "pnpm", ["run", "check:rust:fmt"]],
  ["Rust Clippy", "pnpm", ["run", "check:rust:clippy"]],
  ["Rust tests", "pnpm", ["run", "check:rust:test"]],
  ["Frontend typecheck", "pnpm", ["run", "check:frontend:typecheck"]],
  ["Frontend tests", "pnpm", ["run", "check:frontend:test"]],
  ["Frontend lint", "pnpm", ["run", "check:frontend:lint"]],
  ["Generated binding drift", "pnpm", ["run", "check:bindings"]],
  ["i18n locale drift", "pnpm", ["run", "i18n:check"]],
];

for (const [name, command, args] of steps) {
  console.log(`\n==> ${name}`);
  console.log(`$ ${[command, ...args].join(" ")}`);

  const result = spawnSync(command, args, {
    env: { ...process.env, CI: process.env.CI ?? "1" },
    shell: process.platform === "win32",
    stdio: "inherit",
  });

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

console.log("\nCI baseline checks passed.");
