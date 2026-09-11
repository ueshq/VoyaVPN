import { cpSync, existsSync, mkdirSync, readdirSync, rmSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import {
  capture,
  checkedCapture,
  isCliEntrypoint,
  repoRootFromScript,
  requireDarwin,
  run,
  truthy,
} from "../../lib/common.mjs";
import { libboxBinaryPath } from "./tunnel-layout.mjs";
import { DEFAULT_SING_BOX_VERSION } from "../../core/sing-box-installer.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const sourceDir = resolve(process.env.VOYAVPN_SING_BOX_SOURCE_DIR || resolve(repoRoot, "target", "native", "sing-box"));
const singBoxRef = process.env.VOYAVPN_SING_BOX_REF || process.env.SING_BOX_VERSION || DEFAULT_SING_BOX_VERSION;
const frameworkRoot = resolve(repoRoot, "apps", "desktop", "src-tauri", "native", "macos", "Frameworks");
const targetFramework = resolve(
  process.env.VOYAVPN_LIBBOX_FRAMEWORK || resolve(frameworkRoot, "Libbox.framework"),
);

function ensureSource() {
  if (!existsSync(sourceDir)) {
    mkdirSync(dirname(sourceDir), { recursive: true });
    run("git", ["clone", "https://github.com/SagerNet/sing-box.git", sourceDir], { cwd: repoRoot });
  }

  const status = checkedCapture("git", ["status", "--porcelain"], { cwd: sourceDir }).stdout.trim();
  if (status && !truthy(process.env.VOYAVPN_SING_BOX_ALLOW_DIRTY)) {
    throw new Error(
      `sing-box source checkout has local changes: ${sourceDir}\nSet VOYAVPN_SING_BOX_ALLOW_DIRTY=1 if you intentionally want to build from this checkout.`,
    );
  }

  run("git", ["fetch", "--tags", "--force"], { cwd: sourceDir });
  run("git", ["checkout", singBoxRef], { cwd: sourceDir });
}

function buildLibbox() {
  rmSync(resolve(sourceDir, "Libbox.xcframework"), { force: true, recursive: true });
  run("make", ["lib_install"], { cwd: sourceDir });
  run("make", ["lib_apple"], { cwd: sourceDir });
}

export function findUniversalMacosFramework(xcframeworkPath) {
  const candidates = readdirSync(xcframeworkPath, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && entry.name.toLowerCase().startsWith("macos"))
    .map((entry) => join(xcframeworkPath, entry.name, "Libbox.framework"))
    .filter((path) => existsSync(path));
  if (candidates.length === 0) {
    throw new Error(`Libbox.xcframework has no macOS Libbox.framework slice: ${xcframeworkPath}`);
  }
  candidates.sort((left, right) => Number(/arm64_x86_64|x86_64_arm64/.test(right)) - Number(/arm64_x86_64|x86_64_arm64/.test(left)));
  return candidates[0];
}

export function assertUniversalMacosFramework(frameworkPath, captureCommand = capture) {
  const binary = libboxBinaryPath(frameworkPath);
  if (!existsSync(binary)) {
    throw new Error(`Libbox.framework binary was not found at ${binary}`);
  }
  const result = captureCommand("lipo", ["-archs", binary], { cwd: repoRoot });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`lipo -archs ${binary} failed with status ${result.status}: ${result.stderr ?? ""}`);
  }
  const architectures = new Set((result.stdout ?? "").trim().split(/\s+/).filter(Boolean));
  const missing = ["arm64", "x86_64"].filter((architecture) => !architectures.has(architecture));
  if (missing.length > 0) {
    throw new Error(`Libbox.framework must contain arm64 and x86_64; missing ${missing.join(", ")}: ${binary}`);
  }
}

export function stageLibbox({
  outputXCFramework = resolve(sourceDir, "Libbox.xcframework"),
  destinationFramework = targetFramework,
  captureCommand = capture,
} = {}) {
  if (!existsSync(outputXCFramework) || !statSync(outputXCFramework).isDirectory()) {
    throw new Error(`Libbox.xcframework was not produced at ${outputXCFramework}`);
  }
  const macosFramework = findUniversalMacosFramework(outputXCFramework);
  assertUniversalMacosFramework(macosFramework, captureCommand);

  rmSync(destinationFramework, { force: true, recursive: true });
  mkdirSync(dirname(destinationFramework), { recursive: true });
  cpSync(macosFramework, destinationFramework, {
    dereference: false,
    force: true,
    recursive: true,
    verbatimSymlinks: true,
  });
  assertUniversalMacosFramework(destinationFramework, captureCommand);

  rmSync(outputXCFramework, { force: true, recursive: true });
}

export function main() {
  requireDarwin("Libbox.framework must be built on macOS with Xcode command line tools.");
  ensureSource();
  buildLibbox();
  stageLibbox();
  console.log(`Universal macOS Libbox.framework staged at ${targetFramework}`);
}

if (isCliEntrypoint(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
