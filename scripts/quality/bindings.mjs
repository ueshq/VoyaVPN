import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { repoRootFromScript } from "../lib/common.mjs";
import { generateContractsSource } from "./contracts-source.mjs";

const check = process.argv.includes("--check");
const repoRoot = repoRootFromScript(import.meta.url);
const bindingsPath = resolve(repoRoot, "apps/desktop/src/ipc/bindings.ts");
// Derived from the bindings rather than exported separately: specta emits
// commands, events and types as one file, and `@voya/contracts` is that file
// minus the Tauri plumbing ADR-0002 keeps inside apps/desktop/src/ipc.
const contractsPath = resolve(repoRoot, "packages/contracts/src/generated.ts");

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

function contractsFor(bindingsFile) {
  return generateContractsSource(readFileSync(bindingsFile, "utf8"));
}

if (!check) {
  runExport(bindingsPath);
  writeFileSync(contractsPath, contractsFor(bindingsPath));
  console.log(`Generated ${relative(repoRoot, bindingsPath)}`);
  console.log(`Generated ${relative(repoRoot, contractsPath)}`);
  process.exit(0);
}

for (const path of [bindingsPath, contractsPath]) {
  if (!existsSync(path)) {
    console.error(`Missing generated file at ${relative(repoRoot, path)}`);
    process.exit(1);
  }
}

const tempDir = mkdtempSync(join(tmpdir(), "voyavpn-bindings-"));
const tempPath = join(tempDir, "bindings.ts");

try {
  runExport(tempPath);

  if (readFileSync(bindingsPath, "utf8") !== readFileSync(tempPath, "utf8")) {
    console.error("Generated IPC bindings are out of date. Run `pnpm generate:bindings`.");
    process.exit(1);
  }

  if (readFileSync(contractsPath, "utf8") !== contractsFor(tempPath)) {
    console.error("Generated @voya/contracts source is out of date. Run `pnpm generate:bindings`.");
    process.exit(1);
  }

  console.log("Generated IPC bindings and @voya/contracts are up to date.");
} finally {
  rmSync(tempDir, { force: true, recursive: true });
}
