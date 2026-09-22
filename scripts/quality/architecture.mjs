import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";

import { productionLineCount, splitRustProduction } from "./architecture-analyzer.mjs";
import {
  cargoPackageVersion,
  cargoWorkspaceMembers,
  clashBoundaryRules,
  contractsCasingRule,
  findUndocumentedUnsafe,
  manifestDependencyRules,
  resolveTestModuleFiles,
  retiredCompatibilityRules,
  mobileHostRules,
  shellDtoRule,
  shellRules,
  untranslatedMessageRule,
  gradleVersionName,
  versionAlignmentProblem,
  xcodeMarketingVersion,
  voyaAppRules,
  voyaCoreRules,
} from "./architecture-rules.mjs";

const root = resolve(import.meta.dirname, "../..");
const failures = [];

const rustSources = ["crates", "apps/desktop/src-tauri/src"]
  .flatMap((directory) => walk(resolve(root, directory)))
  .filter((path) => path.endsWith(".rs"))
  .filter((path) => !path.includes("/migrations/"));

// The modules that are actually declared `#[cfg(test)] mod <name>;`, instead
// of every file that happens to be named tests.rs or golden.rs.
const testModuleFiles = resolveTestModuleFiles({
  files: rustSources,
  read: (path) => readFileSync(path, "utf8"),
});
const rustFiles = rustSources.filter((path) => !testModuleFiles.has(path));

// Every file, test modules and inline `mod tests { … }` included, needs its
// SAFETY comments: an undocumented `unsafe` block is just as unsound in a test.
for (const path of rustSources) {
  requireSafetyComments(path, readFileSync(path, "utf8"));
}

// Everything below is about the code that ships, so test code (declared test
// module files, and the terminal test modules `splitRustProduction` strips) is
// exempt from it:
// - the 800-line cap and the terminal-`#[cfg(test)]` layout describe production
//   modules; a test file has no production part to separate;
// - voya-core determinism/OS rules and voya-app adapter rules keep the shipped
//   domain pure and I/O behind adapters, while tests legitimately use tempdirs,
//   std::fs, clocks, and processes to exercise it;
// - the shell rules and DTO rule cannot meet a test file: declaring
//   `#[cfg(test)] mod …` in the shell already fails `shell-tests`;
// - retired v2rayN compatibility and the Clash serde boundary constrain what
//   production parses and serializes, and tests assert the rejection by
//   spelling the retired form (crates/voya-core/src/fmt/tests.rs has
//   `v2rayn://` share links);
// - the untranslated-message hatch is about strings that reach the screen.
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

  if (path.includes("/crates/voya-mobile-ffi/src/")) {
    applyRules(path, source, production, mobileHostRules);
    applyRules(path, source, production, [shellDtoRule]);
  }

  if (!path.endsWith("/crates/voya-net/src/clash.rs")) {
    applyRules(path, source, production, clashBoundaryRules);
  }

  applyRules(path, source, production, retiredCompatibilityRules);

  // The escape hatch from the typed message contract lives in exactly two
  // files: the contract that declares it, and the one mapper that uses it.
  if (
    !path.endsWith("/crates/voya-app/src/contract_map/errors.rs")
    && !path.endsWith("/crates/voya-contracts/src/messages.rs")
  ) {
    applyRules(path, source, production, [untranslatedMessageRule]);
  }
}

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
// Same discipline as the shell, for the same reason: a second host that
// reached into voya-core or voya-db would fork the decisions voya-app owns.
applyManifestRules(
  "crates/voya-mobile-ffi/Cargo.toml",
  manifestDependencyRules(
    "voya-(?:core|db)",
    "the mobile host must not depend directly on voya-core or voya-db",
  ),
);
// The uniffi surface is an envelope, not a model (ADR 0012): the contract is
// `@voya/contracts`, generated from specta in voya-contracts, and a second
// generator over the same types would make it exist twice.
applyManifestRules(
  "crates/voya-mobile-ffi/Cargo.toml",
  manifestDependencyRules("specta", "the mobile host must not depend on Specta"),
);
// The source rules match crate names, so a renamed dependency
// (`http = { package = "reqwest" }`) would slip past them; ban the edge itself.
applyManifestRules(
  "crates/voya-app/Cargo.toml",
  manifestDependencyRules(
    "(?:reqwest|tokio-tungstenite)",
    "voya-app must reach the network through voya-net",
  ),
);

checkReleaseVersionAlignment();

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
  `Architecture checks passed (${rustFiles.length} Rust production files, ${testModuleFiles.size} declared test modules checked for SAFETY comments only).`,
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

function checkReleaseVersionAlignment() {
  const readText = (relativePath) => readFileSync(resolve(root, relativePath), "utf8");
  const workspaceManifest = readText("Cargo.toml");
  const tauriConfigPath = "apps/desktop/src-tauri/tauri.conf.json";
  let tauriVersion = JSON.parse(readText(tauriConfigPath)).version;
  // Tauri also accepts a path to a package.json whose version it should use.
  if (typeof tauriVersion === "string" && tauriVersion.endsWith(".json")) {
    tauriVersion = JSON.parse(readFileSync(resolve(root, dirname(tauriConfigPath), tauriVersion), "utf8")).version;
  }

  const iosProjectPath = "apps/mobile/ios/VoyaVPN.xcodeproj/project.pbxproj";
  const androidGradlePath = "apps/mobile/android/app/build.gradle";

  const problem = versionAlignmentProblem([
    ["package.json", JSON.parse(readText("package.json")).version],
    ["apps/desktop/package.json", JSON.parse(readText("apps/desktop/package.json")).version],
    [tauriConfigPath, tauriVersion],
    // The phones ship the same release. Their versions live where each
    // platform's build reads them: Xcode build settings and the Gradle module.
    ["apps/mobile/package.json", JSON.parse(readText("apps/mobile/package.json")).version],
    [iosProjectPath, xcodeMarketingVersion(readText(iosProjectPath))],
    [androidGradlePath, gradleVersionName(readText(androidGradlePath))],
    ...cargoWorkspaceMembers(workspaceManifest).map((member) => [
      `${member}/Cargo.toml`,
      cargoPackageVersion(readText(`${member}/Cargo.toml`), workspaceManifest),
    ]),
  ]);
  if (problem) failures.push(problem);
}

function requireSafetyComments(path, source) {
  for (const finding of findUndocumentedUnsafe(source)) {
    failures.push(`${display(path)}:${finding.line}: unsafe code requires a nearby SAFETY comment`);
  }
}

function walk(directory) {
  return readdirSync(directory).flatMap((entry) => {
    const path = resolve(directory, entry);
    return statSync(path).isDirectory() ? walk(path) : [path];
  });
}

function display(path) {
  // Forward slashes so messages match on Windows too.
  return relative(root, path).replaceAll("\\", "/");
}
