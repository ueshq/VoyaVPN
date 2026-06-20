import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  probeCandidate,
  validateCoreManifest,
  validateReleaseIndex,
  validateUpdaterMetadata,
  verifyUpdaterMetadataSignatures,
} from "./verify-release-staging.mjs";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const cdnBaseUrl = "https://cdn.voyavpn.dev/stable";
const updatesBaseUrl = "https://updates.voyavpn.dev/stable";
const updaterArtifacts = "tests/fixtures/release/signed-updater";
const updaterPublicKey = readFileSync(resolve(repoRoot, "tests/fixtures/release/updater-signing/public.key"), "utf8").trim();
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
      releaseTargets.map(([releaseTarget]) => {
        const manifestPath = resolve(repoRoot, updaterArtifacts, releaseTarget, "artifact-manifest.json");
        const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
        const payload = manifest.artifacts.find((artifact) => artifact.kind === "updater");
        const signature = readFileSync(resolve(dirname(manifestPath), `${payload.path}.sig`), "utf8").trim();
        return [
          releaseTarget,
          {
            signature,
            url: `${updatesBaseUrl}/${payload.name}`,
          },
        ];
      }),
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

async function listen(server) {
  await new Promise((resolveListen, rejectListen) => {
    server.once("error", rejectListen);
    server.listen(0, "127.0.0.1", () => {
      server.off("error", rejectListen);
      resolveListen();
    });
  });

  const address = server.address();
  if (!address || typeof address === "string") {
    throw new Error("test server did not bind to a TCP port");
  }
  return address.port;
}

function close(server) {
  return new Promise((resolveClose, rejectClose) => {
    server.close((error) => {
      if (error) {
        rejectClose(error);
      } else {
        resolveClose();
      }
    });
  });
}

describe("release staging verification", () => {
  it("accepts complete stable metadata on approved CDN hosts", async () => {
    expect(() => validateReleaseIndex(validReleaseIndex(), { expectedVersion: version })).not.toThrow();
    const latest = validUpdaterMetadata();
    expect(() =>
      validateUpdaterMetadata(latest, {
        expectedVersion: version,
        updatesBaseUrl,
      }),
    ).not.toThrow();
    await expect(
      verifyUpdaterMetadataSignatures(latest, {
        updaterArtifacts,
        env: { VOYAVPN_UPDATER_PUBLIC_KEY: updaterPublicKey },
      }),
    ).resolves.toMatchObject({ verifiedCount: releaseTargets.length });
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

  it("rejects updater metadata signatures that do not verify against the local payload", async () => {
    const latest = validUpdaterMetadata();
    latest.platforms["darwin-aarch64"].signature = latest.platforms["linux-x86_64"].signature;

    await expect(
      verifyUpdaterMetadataSignatures(latest, {
        updaterArtifacts,
        env: { VOYAVPN_UPDATER_PUBLIC_KEY: updaterPublicKey },
      }),
    ).rejects.toThrow(/signature does not match local \.sig artifact|signature verification failed/);
  });

  it("rejects incomplete core asset matrices", () => {
    const manifest = validCoreManifest();
    manifest.assets = manifest.assets.filter((asset) => !(asset.coreType === "sing_box" && asset.os === "linux"));

    expect(() => validateCoreManifest(manifest)).toThrow(/missing required entries/);
  });

  it("rejects redirect responses during CDN probes", async () => {
    const server = createServer((_request, response) => {
      response.writeHead(302, {
        "content-length": "0",
        location: "/ok",
      });
      response.end();
    });
    const port = await listen(server);

    try {
      await expect(
        probeCandidate(
          {
            label: "release-index artifacts[0]",
            url: `http://127.0.0.1:${port}/artifact`,
            bytes: null,
            sha256: null,
          },
          {
            downloadAndHash: false,
            requireCacheHeaders: false,
            timeoutMs: 1000,
          },
        ),
      ).rejects.toThrow(/redirect blocked: 302/);
    } finally {
      await close(server);
    }
  });
});
