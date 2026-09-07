import { readFileSync, readdirSync, statSync } from "node:fs";
import { relative, resolve } from "node:path";

import { productionLineCount, splitRustProduction } from "./architecture-analyzer.mjs";
import {
  clashBoundaryRules,
  contractsCasingRule,
  findUndocumentedUnsafe,
  KNOWN_UNSAFE_WITHOUT_SAFETY_COMMENT,
  manifestDependencyRules,
  resolveTestModuleFiles,
  retiredCompatibilityRules,
  shellDtoRule,
  shellRules,
  voyaAppRules,
  voyaCoreRules,
} from "./architecture-rules.mjs";

const root = resolve(import.meta.dirname, "../..");
const failures = [];

const rustSources = ["crates", "apps/desktop/src-tauri/src"]
  .flatMap((directory) => walk(resolve(root, directory)))
  .filter((path) => path.endsWith(".rs"))
  .filter((path) => !path.includes("/migrations/"));

// Exempt the modules that are actually declared `#[cfg(test)] mod <name>;`
// instead of every file that happens to be named tests.rs or golden.rs.
const testModuleFiles = resolveTestModuleFiles({
  files: rustSources,
  read: (path) => readFileSync(path, "utf8"),
});
const rustFiles = rustSources.filter((path) => !testModuleFiles.has(path));
const usedUnsafeAllowlistEntries = new Set();

for (const path of rustFiles) {
  const source = readFileSync(path, "utf8");
  const { layoutError, production } = splitRustProduction(source);
  if (layoutError) {
    failures.push(`${display(path)}: ${layoutError}`);
  }
  const lineCount = productionLineCount(production);
  if (lineCount > 800) {
    failures.push(`${display(path)} has ${lineCount} production lines (maximum 800)`);
  }

  if (path.includes("/crates/voya-app/src/")) {
    applyRules(path, source, production, voyaAppRules);
  }

  if (path.includes("/crates/voya-core/src/")) {
    applyRules(path, source, production, voyaCoreRules);
  }

  if (path.includes("/apps/desktop/src-tauri/src/") && !path.includes("/src/bin/")) {
    applyRules(path, source, production, shellRules);
    if (!path.endsWith("/ipc/events.rs")) {
      applyRules(path, source, production, [shellDtoRule]);
    }
  }

  requireSafetyComments(path, production);

  if (!path.endsWith("/crates/voya-net/src/clash.rs")) {
    applyRules(path, source, production, clashBoundaryRules);
  }

  applyRules(path, source, production, retiredCompatibilityRules);
}

reportUnusedUnsafeAllowlistEntries();

applyManifestRules(
  "apps/desktop/src-tauri/Cargo.toml",
  manifestDependencyRules(
    "voya-(?:core|db)",
    "the Tauri shell must not depend directly on voya-core or voya-db",
  ),
);
applyManifestRules(
  "crates/voya-app/Cargo.toml",
  manifestDependencyRules("specta", "voya-app must not depend on Specta"),
);

for (const path of walk(resolve(root, "crates/voya-contracts/src")).filter((item) => item.endsWith(".rs"))) {
  const source = readFileSync(path, "utf8");
  applyRules(path, source, source, [contractsCasingRule]);
}

if (failures.length > 0) {
  console.error("Architecture checks failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  `Architecture checks passed (${rustFiles.length} Rust production files, ${testModuleFiles.size} declared test modules exempt).`,
);

function applyRules(path, raw, production, rules) {
  for (const rule of rules) {
    const source = rule.scope === "raw" ? raw : production;
    if (rule.pattern.test(source)) {
      failures.push(`${display(path)}: ${rule.message}`);
    }
  }
}

function applyManifestRules(relativePath, rules) {
  const path = resolve(root, relativePath);
  const source = readFileSync(path, "utf8");
  applyRules(path, source, source, rules);
}

function requireSafetyComments(path, source) {
  const { findings, usedAllowlistEntries } = findUndocumentedUnsafe(source, { path: display(path) });
  for (const index of usedAllowlistEntries) usedUnsafeAllowlistEntries.add(index);
  for (const finding of findings) {
    failures.push(`${display(path)}:${finding.line}: unsafe code requires a nearby SAFETY comment`);
  }
}

function reportUnusedUnsafeAllowlistEntries() {
  // A stale entry is reported, not failed: the owning crate may fix its SAFETY
  // comment at any time, and this gate must not turn red when it does.
  KNOWN_UNSAFE_WITHOUT_SAFETY_COMMENT.forEach((entry, index) => {
    if (usedUnsafeAllowlistEntries.has(index)) return;
    console.warn(
      `Stale unsafe allowlist entry (delete it from architecture-rules.mjs): ${entry.path} — ${entry.line}`,
    );
  });
}

function walk(directory) {
  return readdirSync(directory).flatMap((entry) => {
    const path = resolve(directory, entry);
    return statSync(path).isDirectory() ? walk(path) : [path];
  });
}

function display(path) {
  // Forward slashes so messages and allowlist keys match on Windows too.
  return relative(root, path).replaceAll("\\", "/");
}
