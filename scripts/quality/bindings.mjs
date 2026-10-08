import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { repoRootFromScript, runOrExit } from "../lib/common.mjs";
import {
  generateCommandNames,
  generateCommandWire,
  generateContractsSource,
  generateEventShapes,
} from "./contracts-source.mjs";

const check = process.argv.includes("--check");
const repoRoot = repoRootFromScript(import.meta.url);
const bindingsPath = resolve(repoRoot, "apps/desktop/src/ipc/bindings.ts");
// Derived from the bindings rather than exported separately: specta emits
// commands, events and types as one file, and `@voya/contracts` is that file
// minus the Tauri plumbing ADR-0002 keeps inside apps/desktop/src/ipc.
const contractsPath = resolve(repoRoot, "packages/contracts/src/generated.ts");
// The wire table a non-Tauri transport rebuilds Tauri's named arguments from,
// and the bare command list `crates/voya-mobile-ffi` checks its coverage
// against. Both come out of the same parse as the contract itself.
const commandWirePath = resolve(repoRoot, "packages/contracts/src/commands.ts");
const commandNamesPath = resolve(repoRoot, "packages/contracts/commands.json");
// The three event channels and the `kind` values each payload can carry, for
// the Rust mobile host's own copy of those enums.
const eventShapesPath = resolve(repoRoot, "packages/contracts/events.json");

const derived = [
  { path: contractsPath, render: generateContractsSource },
  { path: commandWirePath, render: generateCommandWire },
  { path: commandNamesPath, render: generateCommandNames },
  { path: eventShapesPath, render: generateEventShapes },
];

function runExport(outputPath) {
  // A cargo example rather than a bin: Tauri bundles every bin target, and this
  // codegen tool has no business in a shipped package.
  runOrExit("cargo", ["run", "-p", "voyavpn", "--example", "export-bindings", "--", outputPath], {
    cwd: repoRoot,
    log: false,
  });
}

if (!check) {
  runExport(bindingsPath);
  const bindings = readFileSync(bindingsPath, "utf8");
  console.log(`Generated ${relative(repoRoot, bindingsPath)}`);
  for (const { path, render } of derived) {
    writeFileSync(path, render(bindings));
    console.log(`Generated ${relative(repoRoot, path)}`);
  }
  process.exit(0);
}

for (const path of [bindingsPath, ...derived.map((entry) => entry.path)]) {
  if (!existsSync(path)) {
    console.error(`Missing generated file at ${relative(repoRoot, path)}`);
    process.exit(1);
  }
}

const tempDir = mkdtempSync(join(tmpdir(), "voyavpn-bindings-"));
const tempPath = join(tempDir, "bindings.ts");

// The exit status is set rather than the process ended inside the `try`:
// `process.exit()` skips `finally`, which left a temp directory behind on
// every failing run — the runs this check exists for.
try {
  runExport(tempPath);

  const bindings = readFileSync(tempPath, "utf8");
  const stale = [
    ...(readFileSync(bindingsPath, "utf8") === bindings ? [] : ["IPC bindings"]),
    ...derived
      .filter(({ path, render }) => readFileSync(path, "utf8") !== render(bindings))
      .map(({ path }) => relative(repoRoot, path)),
  ];
  if (stale.length) {
    console.error(`Out of date: ${stale.join(", ")}. Run \`vp run generate:bindings\`.`);
    process.exitCode = 1;
  } else {
    console.log("Generated IPC bindings and @voya/contracts are up to date.");
  }
} finally {
  rmSync(tempDir, { force: true, recursive: true });
}
