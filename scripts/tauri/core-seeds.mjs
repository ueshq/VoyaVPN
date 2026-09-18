import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { singBoxExecutableName } from "../core/sing-box-installer.mjs";

const requiredBundleResources = {
  "../../../docs/release/THIRD_PARTY_NOTICES.md": "release/THIRD_PARTY_NOTICES.md",
};

const optionalCoreSeedResources = [
  {
    dir: "sing_box",
    source: "resources/core-seeds/sing_box/*",
    target: "core-seeds/sing_box/",
  },
];

/**
 * macOS packages carry no sing-box seed: the PacketTunnel extension links
 * sing-box (Libbox) and is the only core that runs there, so a second copy
 * would only add ~50 MB.
 */
export function platformShipsCoreSeed(platform = process.platform) {
  return platform !== "darwin";
}

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
  if (!platformShipsCoreSeed(platform)) {
    return {};
  }
  const seedRoot = join(repoRoot, "apps", "desktop", "src-tauri", "resources", "core-seeds");
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
