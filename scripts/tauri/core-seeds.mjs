import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { hasStagedRuleSets } from "../core/rule-sets-installer.mjs";
import { singBoxExecutableName } from "../core/sing-box-installer.mjs";

export const requiredBundleResources = {
  "../../../docs/release/THIRD_PARTY_NOTICES.md": "release/THIRD_PARTY_NOTICES.md",
};

const optionalCoreSeedResources = [
  {
    dir: "sing_box",
    isStaged: (dir, platform) => hasExpectedSeedExecutable(dir, platform),
    source: "resources/core-seeds/sing_box/*",
    target: "core-seeds/sing_box/",
  },
  {
    // The default routing profile's rule sets; the app copies them into app
    // data on first run. Only the `.srs` files: the manifest stays behind.
    dir: "rule_sets",
    isStaged: (dir) => hasStagedRuleSets(dir),
    source: "resources/core-seeds/rule_sets/*.srs",
    target: "core-seeds/rule_sets/",
  },
];

export function hasExpectedSeedExecutable(seedDir, platform = process.platform) {
  if (!existsSync(seedDir)) {
    return false;
  }

  const expectedExecutable = singBoxExecutableName(platform).toLowerCase();

  return readdirSync(seedDir, { withFileTypes: true }).some(
    (entry) => entry.isFile() && entry.name.toLowerCase() === expectedExecutable,
  );
}

export function coreSeedBundleResources(repoRoot, { platform = process.platform } = {}) {
  const seedRoot = join(repoRoot, "apps", "desktop", "src-tauri", "resources", "core-seeds");
  const resources = {};

  for (const seed of optionalCoreSeedResources) {
    if (seed.isStaged(join(seedRoot, seed.dir), platform)) {
      resources[seed.source] = seed.target;
    }
  }

  return resources;
}

export function writeOptionalCoreSeedOverlay(repoRoot, overlayPath, options = {}) {
  const seedResources = coreSeedBundleResources(repoRoot, options);
  if (Object.keys(seedResources).length === 0) {
    return null;
  }

  const overlay = {
    bundle: {
      resources: {
        ...requiredBundleResources,
        ...seedResources,
      },
    },
  };

  // Every `pnpm dev` / `tauri build` regenerates the overlay; rewriting identical
  // bytes would only bump its mtime for anything that watches the file.
  const content = `${JSON.stringify(overlay, null, 2)}\n`;
  if (!existsSync(overlayPath) || readFileSync(overlayPath, "utf8") !== content) {
    mkdirSync(dirname(overlayPath), { recursive: true });
    writeFileSync(overlayPath, content);
  }

  return overlayPath;
}
