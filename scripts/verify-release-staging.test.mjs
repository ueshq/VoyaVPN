import { describe, expect, it } from "vitest";

import {
  validateCoreManifest,
  validateReleaseIndex,
  validateUpdaterMetadata,
} from "./verify-release-staging.mjs";

const cdnBaseUrl = "https://cdn.voyavpn.dev/stable";
const updatesBaseUrl = "https://updates.voyavpn.dev/stable";
const version = "0.1.0";
const releaseTargets = [
  ["darwin-aarch64", "macos", "arm64"],
  ["darwin-x86_64", "macos", "x64"],
  ["linux-aarch64", "linux", "arm64"],
  ["linux-x86_64", "linux", "x64"],
  ["windows-aarch64", "windows", "arm64"],
  ["windows-x86_64", "windows", "x64"],
];

function sha(seed) {
  return seed.padEnd(64, seed).slice(0, 64);
}

function validReleaseIndex() {
  return {
    productName: "VoyaVPN",
    channel: "stable",
    version,
    baseUrl: cdnBaseUrl,
    artifacts: releaseTargets.map(([releaseTarget, target, arch], index) => ({
      name: `voyavpn-${version}-stable-${releaseTarget}.pkg`,
      kind: target === "macos" ? "dmg" : "installer",
      target,
      arch,
      channel: "stable",
      version,
      bytes: 1000 + index,
      sha256: sha(String(index + 1)),
      url: `${cdnBaseUrl}/voyavpn-${version}-stable-${releaseTarget}.pkg`,
    })),
  };
}

function validUpdaterMetadata() {
  return {
    version,
    notes: "stable",
    pub_date: "2026-06-06T00:00:00.000Z",
    platforms: Object.fromEntries(
      releaseTargets.map(([releaseTarget]) => [
        releaseTarget,
        {
          signature: `${"a".repeat(86)}==`,
          url: `${updatesBaseUrl}/voyavpn-${version}-stable-${releaseTarget}-updater.zip`,
        },
      ]),
    ),
  };
}

function validCoreManifest() {
  const assets = [];
  let index = 0;
  for (const coreType of ["Xray", "mihomo", "sing_box"]) {
    for (const os of ["windows", "macos", "linux"]) {
      for (const arch of ["x64", "arm64"]) {
        index += 1;
        assets.push({
          coreType,
          version: coreType === "Xray" ? "1.8.24" : "1.0.0",
          license: coreType === "Xray" ? "MPL-2.0" : "GPL-3.0",
          os,
          arch,
          name: `${coreType}-${os}-${arch}.zip`,
          bytes: 2000 + index,
          sha256: sha(index.toString(16)),
          url: `${cdnBaseUrl}/cores/${coreType}/${os}/${arch}/${coreType}.zip`,
          upstreamUrl: `https://github.com/upstream/${coreType}/releases/download/v1/${coreType}.zip`,
        });
      }
    }
  }
  return {
    productName: "VoyaVPN",
    channel: "stable",
    baseUrl: cdnBaseUrl,
    assets,
  };
}

describe("release staging verification", () => {
  it("accepts complete stable metadata on approved CDN hosts", () => {
    expect(() => validateReleaseIndex(validReleaseIndex(), { expectedVersion: version })).not.toThrow();
    expect(() =>
      validateUpdaterMetadata(validUpdaterMetadata(), {
        expectedVersion: version,
        updatesBaseUrl,
      }),
    ).not.toThrow();
    expect(() => validateCoreManifest(validCoreManifest(), { expectedVersion: version })).not.toThrow();
  });

  it("rejects GitHub-hosted release-index artifact URLs", () => {
    const index = validReleaseIndex();
    index.artifacts[0].url = "https://github.com/voyavpn/voyavpn/releases/download/v0.1.0/VoyaVPN.dmg";

    expect(() => validateReleaseIndex(index)).toThrow(/GitHub host/);
  });

  it("rejects placeholder updater signatures and incomplete platform matrices", () => {
    const latest = validUpdaterMetadata();
    latest.platforms["darwin-aarch64"].signature = "VOYAVPN_UPDATER_SIGNATURE_PLACEHOLDER";
    delete latest.platforms["linux-aarch64"];

    expect(() => validateUpdaterMetadata(latest, { updatesBaseUrl })).toThrow(/placeholder|missing stable targets/);
  });

  it("rejects incomplete core asset matrices", () => {
    const manifest = validCoreManifest();
    manifest.assets = manifest.assets.filter((asset) => !(asset.coreType === "sing_box" && asset.os === "linux"));

    expect(() => validateCoreManifest(manifest)).toThrow(/missing required entries/);
  });
});
