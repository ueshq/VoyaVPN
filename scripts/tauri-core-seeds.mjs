import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

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

function hasSeedFiles(seedDir) {
  if (!existsSync(seedDir)) {
    return false;
  }

  return readdirSync(seedDir, { withFileTypes: true }).some((entry) => entry.isFile() && !entry.name.startsWith("."));
}

export function coreSeedBundleResources(repoRoot) {
  const seedRoot = join(repoRoot, "src-tauri", "resources", "core-seeds");
  const resources = {};

  for (const seed of optionalCoreSeedResources) {
    if (hasSeedFiles(join(seedRoot, seed.dir))) {
      resources[seed.source] = seed.target;
    }
  }

  return resources;
}

export function writeOptionalCoreSeedOverlay(repoRoot, overlayPath) {
  const seedResources = coreSeedBundleResources(repoRoot);
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
