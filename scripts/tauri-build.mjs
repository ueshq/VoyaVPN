import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const args = ["build", ...process.argv.slice(2)];
const env = { ...process.env };
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const localTauriBin = resolve(
  repoRoot,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "tauri.cmd" : "tauri",
);
const tauriBin = existsSync(localTauriBin) ? localTauriBin : "tauri";

if (env.CI === "1") {
  env.CI = "true";
} else if (env.CI === "0") {
  env.CI = "false";
}

const child = spawn(tauriBin, args, {
  env,
  shell: process.platform === "win32",
  stdio: "inherit",
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 1);
});

child.on("error", (error) => {
  console.error(error.message);
  process.exit(1);
});
