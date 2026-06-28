import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { xrayExecutableName } from "./xray-core-installer.mjs";

const requiredBundleResources = {
  "../docs/release/THIRD_PARTY_NOTICES.md": "release/THIRD_PARTY_NOTICES.md",
};

const optionalCoreSeedResources = [
  {
    dir: "xray",
    source: "resources/core-seeds/xray/*",
    target: "core-seeds/xray/",
  },
];

export function hasExpectedSeedExecutable(seedDir, platform = process.platform) {
  if (!existsSync(seedDir)) {
    return false;
  }

  const expectedExecutable = xrayExecutableName(platform).toLowerCase();

  return readdirSync(seedDir, { withFileTypes: true }).some(
    (entry) => entry.isFile() && entry.name.toLowerCase() === expectedExecutable,
  );
}

export function coreSeedBundleResources(repoRoot, { platform = process.platform } = {}) {
  const seedRoot = join(repoRoot, "src-tauri", "resources", "core-seeds");
  const resources = {};

  for (const seed of optionalCoreSeedResources) {
    if (hasExpectedSeedExecutable(join(seedRoot, seed.dir), platform)) {
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

  mkdirSync(dirname(overlayPath), { recursive: true });
  writeFileSync(overlayPath, `${JSON.stringify(overlay, null, 2)}\n`);

  return overlayPath;
}
