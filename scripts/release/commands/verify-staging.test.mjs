import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  assertUpdaterSignatureCoverage,
  probeCandidate,
  validateCoreManifest,
  validateReleaseIndex,
  validateUpdaterMetadata,
  verifyUpdaterMetadataSignatures,
} from "./verify-staging.mjs";
import { stableTargets } from "../matrix.mjs";
import { selectUpdaterPayload } from "../validation.mjs";

const repoRoot = resolve(fileURLToPath(new URL("../../..", import.meta.url)));
const cdnBaseUrl = "https://cdn.voyavpn.dev/stable";
const updatesBaseUrl = "https://updates.voyavpn.dev/stable";
const updaterArtifacts = "tests/fixtures/release/signed-updater";
const updaterPublicKey = readFileSync(resolve(repoRoot, "tests/fixtures/release/updater-signing/public.key"), "utf8").trim();
const version = "0.1.0";
const releaseTargets = stableTargets
  .map((target) => [target.releaseTarget, target.os, target.arch])
  .sort(([left], [right]) => left.localeCompare(right));

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
        const payload = selectUpdaterPayload(manifest.artifacts);
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
  return {
    productName: "VoyaVPN",
    channel: "stable",
    baseUrl: cdnBaseUrl,
    assets: [],
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
    expect(() => validateReleaseIndex(validReleaseIndex(), { cdnBaseUrl, expectedVersion: version })).not.toThrow();
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
    expect(() => validateCoreManifest(validCoreManifest(), { cdnBaseUrl, expectedVersion: version })).not.toThrow();
  });

  it("refuses to let the metadata under test declare its own approved CDN base", () => {
    const index = validReleaseIndex();

    expect(() => validateReleaseIndex(index, { expectedVersion: version })).toThrow(
      /CDN base URL is required for release-index validation/,
    );
    expect(() => validateCoreManifest(validCoreManifest(), {})).toThrow(
      /CDN base URL is required for core manifest validation/,
    );

    index.baseUrl = "https://cdn.voyavpn.dev/other-stable";
    expect(() => validateReleaseIndex(index, { cdnBaseUrl })).toThrow(/baseUrl must be the approved/);
  });

  it("rejects GitHub-hosted release-index artifact URLs", () => {
    const index = validReleaseIndex();
    index.artifacts[0].url = "https://github.com/voyavpn/voyavpn/releases/download/v0.1.0/VoyaVPN.dmg";

    expect(() => validateReleaseIndex(index, { cdnBaseUrl })).toThrow(/GitHub host/);
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

  it.each([null, {}, "", 42])("rejects a malformed core asset list: %j", (assets) => {
    expect(() => validateCoreManifest({ ...validCoreManifest(), assets }, { cdnBaseUrl }))
      .toThrow(/assets must be an array/);
  });

  it("rejects downloadable core asset entries", () => {
    const manifest = validCoreManifest();
    manifest.assets.push({
      coreType: "legacy-core",
      version: "1.0.0",
      license: "GPL-3.0",
      os: "linux",
      arch: "x64",
      name: "legacy-core-linux-x64.gz",
      bytes: 2000,
      sha256: sha("a"),
      url: `${cdnBaseUrl}/cores/legacy-core/linux/x64/legacy-core.gz`,
      upstreamUrl: "https://source.example.invalid/legacy-core/releases/v1",
    });

    expect(() => validateCoreManifest(manifest, { cdnBaseUrl })).toThrow(/not supported/);
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

describe("updater signature verification during download-and-hash", () => {
  const target = "linux-x86_64";
  const fixtureDir = resolve(repoRoot, updaterArtifacts, target);
  const manifest = JSON.parse(readFileSync(resolve(fixtureDir, "artifact-manifest.json"), "utf8"));
  const payloadArtifact = selectUpdaterPayload(manifest.artifacts);
  const payload = readFileSync(resolve(fixtureDir, payloadArtifact.path));
  const signature = readFileSync(resolve(fixtureDir, `${payloadArtifact.path}.sig`), "utf8").trim();

  function payloadServer(body) {
    return createServer((_request, response) => {
      response.writeHead(200, {
        "cache-control": "public, max-age=300",
        "content-length": String(body.length),
        "content-type": "application/zip",
      });
      response.end(body);
    });
  }

  function updaterCandidate(port) {
    return {
      label: `latest.json platforms.${target}`,
      url: `http://127.0.0.1:${port}/${payloadArtifact.name}`,
      bytes: null,
      sha256: null,
      updaterSignature: signature,
      updaterTarget: target,
    };
  }

  async function probeUpdaterPayload(body, options) {
    const server = payloadServer(body);
    const port = await listen(server);
    try {
      return await probeCandidate(updaterCandidate(port), {
        downloadAndHash: true,
        requireCacheHeaders: false,
        timeoutMs: 5000,
        ...options,
      });
    } finally {
      await close(server);
    }
  }

  it("downloads updater payloads that carry a signature instead of a sha256", async () => {
    const result = await probeUpdaterPayload(payload, { updaterPublicKey });

    expect(result.checked).toBe("download-and-hash");
    expect(result.bytes).toBe(payload.length);
    expect(result.updaterSignature).toMatchObject({ keyId: expect.any(String) });
  });

  it("fails when the CDN payload does not match the latest.json signature", async () => {
    await expect(probeUpdaterPayload(Buffer.concat([payload, Buffer.from("tampered")]), { updaterPublicKey })).rejects.toThrow(
      /signature verification failed/,
    );
  });

  it("refuses to download updater payloads without an approved updater public key", async () => {
    await expect(probeUpdaterPayload(payload, { updaterPublicKey: null })).rejects.toThrow(
      /without an approved updater public key/,
    );
  });

  it("requires one verified signature per updater platform before reporting a pass", () => {
    const verified = { label: "latest.json platforms.linux-x86_64", updaterSignature: { keyId: "AA" } };
    const probed = { label: "release-index artifacts[0]", checked: "download-and-hash" };

    expect(assertUpdaterSignatureCoverage([verified, verified, probed], 2)).toBe(2);
    expect(() => assertUpdaterSignatureCoverage([verified, probed], 2)).toThrow(
      /verified 1 of 2 updater platform signatures/,
    );
  });
});
