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
  const invocation = executable(command, args);

  const result = spawnSync(invocation.file, invocation.args, {
    env: { ...process.env, CI: process.env.CI ?? "true" },
    stdio: "inherit",
  });

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

console.log("\nCI baseline checks passed.");

function executable(command, args) {
  if (command === "pnpm" && process.env.npm_execpath) {
    return { file: process.execPath, args: [process.env.npm_execpath, ...args] };
  }

  return { file: command, args };
}
