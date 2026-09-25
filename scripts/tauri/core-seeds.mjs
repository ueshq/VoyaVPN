import { join } from "node:path";
import { hasStagedRuleSets } from "../core/rule-sets-installer.mjs";
import { coreSeedsDir, hasExpectedSingBoxExecutable } from "../core/sing-box-installer.mjs";
import { writeJson } from "../lib/fs.mjs";

export const requiredBundleResources = {
  "../../../docs/release/THIRD_PARTY_NOTICES.md": "release/THIRD_PARTY_NOTICES.md",
};

const optionalCoreSeedResources = [
  {
    dir: "sing_box",
    isStaged: (dir, platform) => hasExpectedSingBoxExecutable(dir, platform),
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

export function coreSeedBundleResources(repoRoot, { platform = process.platform } = {}) {
  const seedRoot = coreSeedsDir(repoRoot);
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

  // Every `pnpm dev` / `tauri build` regenerates the overlay.
  writeJson(overlayPath, overlay, { onlyIfChanged: true });

  return overlayPath;
}
