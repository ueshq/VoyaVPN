import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, rmSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { DEFAULT_SING_BOX_VERSION } from "./sing-box-core-installer.mjs";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourceDir = resolve(process.env.VOYAVPN_SING_BOX_SOURCE_DIR || resolve(repoRoot, "target", "native", "sing-box"));
const singBoxRef = process.env.VOYAVPN_SING_BOX_REF || process.env.SING_BOX_VERSION || DEFAULT_SING_BOX_VERSION;
const targetXCFramework = resolve(
  process.env.VOYAVPN_LIBBOX_XCFRAMEWORK ||
    resolve(repoRoot, "src-tauri", "native", "macos", "Frameworks", "Libbox.xcframework"),
);

function run(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: options.cwd ?? repoRoot,
    env: { ...process.env, ...options.env },
    stdio: "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")} failed with status ${result.status}`);
  }
}

function runText(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: options.cwd ?? repoRoot,
    encoding: "utf8",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")} failed with status ${result.status}: ${result.stderr}`);
  }
  return result.stdout;
}

function truthy(value) {
  return /^(1|true|yes|on)$/i.test(String(value ?? "").trim());
}

function requireDarwin() {
  if (process.platform !== "darwin") {
    throw new Error("Libbox.xcframework must be built on macOS with Xcode command line tools.");
  }
}

function ensureSource() {
  if (!existsSync(sourceDir)) {
    mkdirSync(dirname(sourceDir), { recursive: true });
    run("git", ["clone", "https://github.com/SagerNet/sing-box.git", sourceDir]);
  }

  const status = runText("git", ["status", "--porcelain"], { cwd: sourceDir }).trim();
  if (status && !truthy(process.env.VOYAVPN_SING_BOX_ALLOW_DIRTY)) {
    throw new Error(
      `sing-box source checkout has local changes: ${sourceDir}\nSet VOYAVPN_SING_BOX_ALLOW_DIRTY=1 if you intentionally want to build from this checkout.`,
    );
  }

  run("git", ["fetch", "--tags", "--force"], { cwd: sourceDir });
  run("git", ["checkout", singBoxRef], { cwd: sourceDir });
}

function buildLibbox() {
  run("make", ["lib_install"], { cwd: sourceDir });
  run("make", ["lib_apple"], { cwd: sourceDir });
}

function stageLibbox() {
  const output = resolve(sourceDir, "Libbox.xcframework");
  if (!existsSync(output) || !statSync(output).isDirectory()) {
    throw new Error(`Libbox.xcframework was not produced at ${output}`);
  }

  rmSync(targetXCFramework, { force: true, recursive: true });
  mkdirSync(dirname(targetXCFramework), { recursive: true });
  cpSync(output, targetXCFramework, {
    dereference: false,
    force: true,
    recursive: true,
    verbatimSymlinks: true,
  });
}

function main() {
  requireDarwin();
  ensureSource();
  buildLibbox();
  stageLibbox();
  console.log(`Libbox.xcframework staged at ${targetXCFramework}`);
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
