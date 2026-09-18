import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { repoRootFromScript } from "../lib/common.mjs";

const check = process.argv.includes("--check");
const repoRoot = repoRootFromScript(import.meta.url);
const bindingsPath = resolve(repoRoot, "apps/desktop/src/ipc/bindings.ts");

function runExport(outputPath) {
  // A cargo example rather than a bin: Tauri bundles every bin target, and this
  // codegen tool has no business in a shipped package.
  const result = spawnSync("cargo", ["run", "-p", "voyavpn", "--example", "export-bindings", "--", outputPath], {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: "inherit",
  });

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

if (!check) {
  runExport(bindingsPath);
  console.log(`Generated ${relative(repoRoot, bindingsPath)}`);
  process.exit(0);
}

if (!existsSync(bindingsPath)) {
  console.error(`Missing generated bindings at ${relative(repoRoot, bindingsPath)}`);
  process.exit(1);
}

const tempDir = mkdtempSync(join(tmpdir(), "voyavpn-bindings-"));
const tempPath = join(tempDir, "bindings.ts");

try {
  runExport(tempPath);

  const current = readFileSync(bindingsPath, "utf8");
  const generated = readFileSync(tempPath, "utf8");

  if (current !== generated) {
    console.error("Generated IPC bindings are out of date. Run `pnpm generate:bindings`.");
    process.exit(1);
  }

  console.log("Generated IPC bindings are up to date.");
} finally {
  rmSync(tempDir, { force: true, recursive: true });
}
