import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { promisify } from "node:util";
import {
  isPositiveByteSize,
  isSha256Hex,
  isUrlDerivedFromBase,
  missingExpectedValues,
  placeholderText,
  sha256File,
  sha256Text,
  walkArtifactManifests,
} from "../../validation.mjs";
import { stableTargets } from "../../matrix.mjs";
import {
  repoRoot,
  resolveRepoPath,
  readJson,
  displayPath,
  isDryRun,
  stableInputPath,
  stableInputPathAny,
  assertStableEvidencePath,
  assert,
  forbiddenSerialized,
} from "./inputs.mjs";
import { lineSummary } from "./reporter.mjs";
const execFileAsync = promisify(execFile);

async function runGenerator(script, args, env) {
  const result = await execFileAsync(process.execPath, [resolveRepoPath(script), ...args], {
    cwd: repoRoot,
    env: { ...process.env, ...env },
    maxBuffer: 10 * 1024 * 1024,
  });

  return [...result.stdout.trim().split(/\r?\n/), ...result.stderr.trim().split(/\r?\n/)].filter(Boolean);
}

function validateReleaseIndex(index, cdnBaseUrl) {
  assert(index.productName === "VoyaVPN", "release index productName must be VoyaVPN");
  assert(index.channel === "stable", "release index channel must be stable");
  assert(index.baseUrl === cdnBaseUrl, "release index baseUrl must match readiness CDN base URL");
  assert(Array.isArray(index.artifacts) && index.artifacts.length > 0, "release index artifacts[] must be non-empty");
  assert(!forbiddenSerialized(index), "release index contains placeholder, example, or GitHub content");

  const present = new Set(index.artifacts.map((artifact) => `${artifact.target}/${artifact.arch}`));
  const missing = missingExpectedValues(
    stableTargets.map((target) => `${target.os}/${target.arch}`),
    present,
  );
  assert(missing.length === 0, `release index is missing first-stable target(s): ${missing.join(", ")}`);

  for (const artifact of index.artifacts) {
    assert(isUrlDerivedFromBase(artifact.url, cdnBaseUrl), `release index URL is not CDN-derived: ${artifact.url}`);
    assert(isPositiveByteSize(artifact.bytes), `release index artifact has invalid bytes: ${artifact.name}`);
    assert(isSha256Hex(artifact.sha256), `release index artifact has invalid sha256: ${artifact.name}`);
  }
}

function validateUpdaterMetadata(latest, updatesBaseUrl) {
  assert(typeof latest.version === "string" && latest.version.length > 0, "latest.json version is missing");
  assert(typeof latest.pub_date === "string" && latest.pub_date.length > 0, "latest.json pub_date is missing");
  assert(latest.platforms && typeof latest.platforms === "object", "latest.json platforms object is missing");
  assert(!forbiddenSerialized(latest), "latest.json contains placeholder, example, or GitHub content");

  const keys = Object.keys(latest.platforms).sort((left, right) => left.localeCompare(right));
  const missing = missingExpectedValues(stableTargets.map((target) => target.updater), new Set(keys));
  assert(missing.length === 0, `latest.json is missing first-stable updater target(s): ${missing.join(", ")}`);

  for (const [target, platform] of Object.entries(latest.platforms)) {
    assert(isUrlDerivedFromBase(platform.url, updatesBaseUrl), `updater URL for ${target} is not base-url-derived`);
    assert(!placeholderText(platform.signature), `updater signature for ${target} is a placeholder`);
    assert(String(platform.signature).length >= 32, `updater signature for ${target} is too short`);
  }
}

function validateCoreManifest(manifest, cdnBaseUrl) {
  assert(manifest.productName === "VoyaVPN", "core manifest productName must be VoyaVPN");
  assert(manifest.channel === "stable", "core manifest channel must be stable");
  assert(manifest.baseUrl === cdnBaseUrl, "core manifest baseUrl must match readiness CDN base URL");
  assert(Array.isArray(manifest.assets), "core manifest assets[] must be an array");

  assert(manifest.assets.length === 0, "downloadable core assets are not supported; sing-box is bundled with the application");
}

function safeRelativePath(value, context) {
  if (!value || typeof value !== "string") {
    throw new Error(`${context} path is missing`);
  }

  const normalized = value.replaceAll("\\", "/");
  if (normalized.startsWith("/") || normalized.split("/").some((segment) => segment === "..")) {
    throw new Error(`${context} path is unsafe: ${value}`);
  }
  return normalized;
}

export function validateStableUpdaterConfigMetadata(metadata, { updatesBaseUrl, updaterPublicKey, label }) {
  assert(metadata && typeof metadata === "object", `${label} stableUpdaterConfig is missing`);
  assert(/^[a-f0-9]{64}$/i.test(metadata.sha256 ?? ""), `${label} stableUpdaterConfig.sha256 is invalid`);
  assert(metadata.createUpdaterArtifacts === true, `${label} stable updater artifacts flag is not enabled`);

  const expectedPubkeySha256 = sha256Text(String(updaterPublicKey ?? "").trim());
  assert(
    metadata.pubkeySha256 === expectedPubkeySha256,
    `${label} stable updater public key hash does not match VOYAVPN_UPDATER_PUBLIC_KEY`,
  );

  assert(Array.isArray(metadata.endpoints) && metadata.endpoints.length > 0, `${label} updater endpoints are missing`);
  const expectedEndpoint = `${updatesBaseUrl}/latest.json`;
  for (const endpoint of metadata.endpoints) {
    assert(endpoint === expectedEndpoint, `${label} updater endpoint does not match readiness base URL: ${endpoint}`);
  }

  if (metadata.path !== undefined) {
    safeRelativePath(metadata.path, `${label} stableUpdaterConfig`);
  }
}

async function checkStableUpdaterConfigEvidence(reporter, options, roots, updatesBaseUrl, expectedConfigSha256) {
  if (isDryRun(options)) {
    reporter.pass("packaged updater config evidence", [
      "dry-run mode does not require package-time stable updater overlay evidence",
    ]);
    return;
  }

  const updaterPublicKey = process.env.VOYAVPN_UPDATER_PUBLIC_KEY ?? process.env.TAURI_UPDATER_PUBLIC_KEY ?? "";
  const manifestPaths = [];
  for (const root of roots) {
    manifestPaths.push(...(await walkArtifactManifests(resolveRepoPath(root))));
  }

  if (manifestPaths.length === 0) {
    reporter.fail("packaged updater config evidence", ["no artifact-manifest.json files found"]);
    return;
  }

  const failures = [];
  const overlayHashes = new Set();
  const copiedOverlayHashes = new Set();

  for (const manifestPath of manifestPaths) {
    const label = displayPath(manifestPath);
    const manifest = await readJson(manifestPath);
    try {
      validateStableUpdaterConfigMetadata(manifest.stableUpdaterConfig, {
        updatesBaseUrl,
        updaterPublicKey,
        label,
      });
      overlayHashes.add(manifest.stableUpdaterConfig.sha256);

      if (manifest.stableUpdaterConfig.path) {
        const copiedPath = resolve(dirname(manifestPath), safeRelativePath(manifest.stableUpdaterConfig.path, label));
        const copiedHash = await sha256File(copiedPath);
        copiedOverlayHashes.add(copiedHash);
        if (copiedHash !== manifest.stableUpdaterConfig.sha256) {
          failures.push(`${label} stable updater config copy hash does not match artifact-manifest.json`);
        }
      }
    } catch (error) {
      failures.push(error.message);
    }
  }

  if (overlayHashes.size > 1) {
    failures.push(`package targets used different stable updater overlay hashes: ${[...overlayHashes].join(", ")}`);
  }
  if (expectedConfigSha256 && !overlayHashes.has(expectedConfigSha256)) {
    failures.push(
      `checked Tauri updater config hash ${expectedConfigSha256} does not match package-time overlay hash ${[
        ...overlayHashes,
      ].join(", ")}`,
    );
  }
  if (copiedOverlayHashes.size > 1) {
    failures.push(`package targets uploaded different stable updater overlay files: ${[...copiedOverlayHashes].join(", ")}`);
  }

  if (failures.length > 0) {
    reporter.fail("packaged updater config evidence", lineSummary(failures, 10));
    return;
  }

  reporter.pass("packaged updater config evidence", [
    `validated ${manifestPaths.length} artifact manifest(s)`,
    `overlay sha256: ${[...overlayHashes][0]}`,
    ...(expectedConfigSha256 ? ["checked Tauri config hash matches package-time overlay"] : []),
  ]);
}

export async function checkGeneratedManifests(reporter, options, cdnBaseUrl, updatesBaseUrl, workDir) {
  const releaseArtifacts = stableInputPath(
    options,
    options.releaseArtifacts,
    "VOYAVPN_RELEASE_ARTIFACTS_DIR",
    isDryRun(options) ? "tests/fixtures/release/artifacts" : "dist/release/artifacts",
  );
  const updaterArtifacts = stableInputPath(
    options,
    options.updaterArtifacts,
    "VOYAVPN_SIGNED_UPDATER_DIR",
    isDryRun(options) ? "tests/fixtures/release/signed-updater" : "dist/release/signed-updater",
  );
  const coreAssets = stableInputPathAny(
    options.coreAssets,
    ["VOYAVPN_CORE_ASSETS_FILE", "VOYAVPN_CORE_ASSETS_FIXTURE"],
    isDryRun(options) ? "tests/fixtures/release/core-assets.json" : "dist/release/core-assets/source-core-assets.json",
  );

  const releaseIndexOut = join(workDir, "release-index.json");
  const latestOut = join(workDir, "latest.json");
  const coreManifestOut = join(workDir, "core-assets.json");
  const env = {
    VOYAVPN_CDN_BASE_URL: cdnBaseUrl,
    VOYAVPN_UPDATES_BASE_URL: updatesBaseUrl,
  };
  if (isDryRun(options)) {
    const fixtureUpdaterPublicKey = (
      await readFile(resolveRepoPath("tests/fixtures/release/updater-signing/public.key"), "utf8")
    ).trim();
    env.VOYAVPN_UPDATER_PUBLIC_KEY = fixtureUpdaterPublicKey;
    env.TAURI_UPDATER_PUBLIC_KEY = fixtureUpdaterPublicKey;
  }

  assertStableEvidencePath(options, "release artifacts", releaseArtifacts);
  assertStableEvidencePath(options, "signed updater artifacts", updaterArtifacts);
  assertStableEvidencePath(options, "core asset source", coreAssets);
  const expectedConfigSha256 = isDryRun(options) ? null : await sha256File(resolveRepoPath(options.tauriConfig));
  await checkStableUpdaterConfigEvidence(
    reporter,
    options,
    [...new Set([releaseArtifacts, updaterArtifacts])],
    updatesBaseUrl,
    expectedConfigSha256,
  );
  if (options.releaseIndex) {
    assertStableEvidencePath(options, "release index evidence", options.releaseIndex);
    const releaseIndexEvidence = await readJson(resolveRepoPath(options.releaseIndex));
    validateReleaseIndex(releaseIndexEvidence, cdnBaseUrl);
    reporter.pass("workflow CDN staging metadata", [`validated ${options.releaseIndex}`]);
  }
  if (options.updaterMetadata) {
    assertStableEvidencePath(options, "updater metadata evidence", options.updaterMetadata);
    const updaterMetadataEvidence = await readJson(resolveRepoPath(options.updaterMetadata));
    validateUpdaterMetadata(updaterMetadataEvidence, updatesBaseUrl);
    reporter.pass("workflow updater metadata", [`validated ${options.updaterMetadata}`]);
  }
  if (options.coreManifest) {
    assertStableEvidencePath(options, "core manifest evidence", options.coreManifest);
    const coreManifestEvidence = await readJson(resolveRepoPath(options.coreManifest));
    validateCoreManifest(coreManifestEvidence, cdnBaseUrl);
    reporter.pass("workflow core manifest metadata", [`validated ${options.coreManifest}`]);
  }

  const releaseOutput = await runGenerator(
    "scripts/release/cli.mjs",
    ["index", "--input", releaseArtifacts, "--out", releaseIndexOut, "--base-url", cdnBaseUrl, "--channel", "stable"],
    env,
  );
  const releaseIndex = await readJson(releaseIndexOut);
  validateReleaseIndex(releaseIndex, cdnBaseUrl);
  reporter.pass("stable release index manifest", [
    `generated ${displayPath(releaseIndexOut)}`,
    `artifacts: ${releaseIndex.artifacts.length}`,
    ...lineSummary(releaseOutput, 2).map((line) => line.trim()),
  ]);

  const updaterOutput = await runGenerator(
    "scripts/release/cli.mjs",
    ["updater", "--input", updaterArtifacts, "--out", latestOut, "--channel", "stable", "--base-url", updatesBaseUrl],
    env,
  );
  const latest = await readJson(latestOut);
  validateUpdaterMetadata(latest, updatesBaseUrl);
  reporter.pass("stable updater metadata", [
    `generated ${displayPath(latestOut)}`,
    `platforms: ${Object.keys(latest.platforms).length}`,
    ...lineSummary(updaterOutput, 2).map((line) => line.trim()),
  ]);

  const coreOutput = await runGenerator(
    "scripts/release/cli.mjs",
    ["core-assets", "--fixture", coreAssets, "--out", coreManifestOut, "--base-url", cdnBaseUrl, "--channel", "stable"],
    env,
  );
  const coreManifest = await readJson(coreManifestOut);
  validateCoreManifest(coreManifest, cdnBaseUrl);
  reporter.pass("stable core asset manifest", [
    `generated ${displayPath(coreManifestOut)}`,
    `assets: ${coreManifest.assets.length}`,
    ...lineSummary(coreOutput, 2).map((line) => line.trim()),
  ]);
}
