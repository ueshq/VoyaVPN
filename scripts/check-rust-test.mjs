import { spawnSync } from "node:child_process";

function run(command, args, options = {}) {
  console.log(`$ ${[command, ...args].join(" ")}`);
  const result = spawnSync(command, args, {
    ...options,
    stdio: "inherit",
  });

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

run("cargo", ["test", "--workspace", "--all-targets", "--exclude", "voyavpn"]);
run("cargo", ["test", "-p", "voyavpn", "--bin", "voyavpn"]);
