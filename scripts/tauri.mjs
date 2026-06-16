import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const localTauriJs = resolve(
  repoRoot,
  "node_modules",
  "@tauri-apps",
  "cli",
  "tauri.js",
);
const args = process.argv.slice(2);
const tauriArgs = args.length === 0 ? ["dev"] : args;
const command = existsSync(localTauriJs) ? process.execPath : "tauri";
const commandArgs = existsSync(localTauriJs)
  ? [localTauriJs, ...tauriArgs]
  : tauriArgs;

const child = spawn(command, commandArgs, {
  env: process.env,
  shell: false,
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
