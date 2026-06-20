import { createHash } from "node:crypto";
import { readdir, readFile, stat } from "node:fs/promises";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  resolveApprovedUpdaterPublicKey,
  verifyTauriUpdaterSignature,
  verifyTauriUpdaterSignatureFile,
} from "./updater-signatures.mjs";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const stableTargets = [
  "darwin-aarch64",
  "darwin-x86_64",
  "linux-aarch64",
  "linux-x86_64",
  "windows-aarch64",
  "windows-x86_64",
];
const stableTargetSet = new Set(stableTargets);
const stableCoreTypes = ["Xray", "mihomo", "sing_box"];
const stableOs = ["windows", "macos", "linux"];
const stableArchs = ["x64", "arm64"];
const targetMatrix = [
  { releaseTarget: "darwin-x86_64", target: "macos", arch: "x64" },
  { releaseTarget: "darwin-aarch64", target: "macos", arch: "arm64" },
  { releaseTarget: "windows-x86_64", target: "windows", arch: "x64" },
  { releaseTarget: "windows-aarch64", target: "windows", arch: "arm64" },
  { releaseTarget: "linux-x86_64", target: "linux", arch: "x64" },
  { releaseTarget: "linux-aarch64", target: "linux", arch: "arm64" },
];

class StagingValidationError extends Error {
  constructor(label, failures) {
    super(`${label} failed:\n- ${failures.join("\n- ")}`);
    this.name = "StagingValidationError";
    this.failures = failures;
  }
}

function parseArgs(argv) {
  const options = {
    releaseIndex: "dist/release/release-index.json",
    updaterMetadata: "dist/release/latest.json",
    updaterArtifacts: process.env.VOYAVPN_SIGNED_UPDATER_DIR ?? "dist/release/signed-updater",
    coreManifest: "dist/release/core-assets.json",
    cdnBaseUrl: process.env.VOYAVPN_CDN_BASE_URL ?? null,
    updatesBaseUrl: process.env.VOYAVPN_UPDATES_BASE_URL ?? null,
    expectedVersion: null,
    allowTestHosts: false,
    probe: false,
    downloadAndHash: false,
    requireCacheHeaders: false,
    timeoutMs: 15_000,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = () => {
      const value = argv[index + 1];
      if (!value || value.startsWith("--")) {
        throw new Error(`${arg} requires a value`);
      }
      index += 1;
      return value;
    };

    switch (arg) {
      case "--release-index":
        options.releaseIndex = next();
        break;
      case "--updater-metadata":
      case "--latest":
        options.updaterMetadata = next();
        break;
      case "--updater-artifacts":
        options.updaterArtifacts = next();
        break;
      case "--core-manifest":
      case "--core-assets":
        options.coreManifest = next();
        break;
      case "--cdn-base-url":
        options.cdnBaseUrl = next();
        break;
      case "--updates-base-url":
        options.updatesBaseUrl = next();
        break;
      case "--expected-version":
      case "--version":
        options.expectedVersion = next();
        break;
      case "--timeout-ms":
        options.timeoutMs = Number.parseInt(next(), 10);
        break;
      case "--allow-test-hosts":
        options.allowTestHosts = true;
        break;
      case "--probe":
        options.probe = true;
        break;
      case "--download-and-hash":
        options.downloadAndHash = true;
        options.probe = true;
        break;
      case "--require-cache-headers":
        options.requireCacheHeaders = true;
        options.probe = true;
        break;
      case "--skip-release-index":
        options.releaseIndex = null;
        break;
      case "--skip-updater-metadata":
        options.updaterMetadata = null;
        break;
      case "--skip-updater-artifacts":
        options.updaterArtifacts = null;
        break;
      case "--skip-core-manifest":
        options.coreManifest = null;
        break;
      case "--help":
        printHelp();
        process.exit(0);
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  if (!Number.isFinite(options.timeoutMs) || options.timeoutMs <= 0) {
    throw new Error("--timeout-ms must be a positive integer");
  }

  return options;
}

function printHelp() {
  console.log(`Usage: node scripts/verify-release-staging.mjs [options]

Validates stable release metadata before CDN pointer promotion. By default it
checks local generated metadata files and performs no network access. Add
--probe to issue HEAD/range requests, or --download-and-hash to download each
referenced asset and verify SHA-256 values.

Options:
  --release-index <file|url>      Manual download release-index.json.
                                  Default: dist/release/release-index.json
  --updater-metadata <file|url>   Tauri latest.json metadata.
                                  Default: dist/release/latest.json
  --updater-artifacts <dir>       Local signed updater artifact root used for offline signature verification.
                                  Default: VOYAVPN_SIGNED_UPDATER_DIR or dist/release/signed-updater
  --core-manifest <file|url>      Core asset manifest.
                                  Default: dist/release/core-assets.json
  --cdn-base-url <url>            Approved CDN base URL. Defaults to VOYAVPN_CDN_BASE_URL
  --updates-base-url <url>        Approved updater base URL. Defaults to VOYAVPN_UPDATES_BASE_URL
  --expected-version <version>    Require all app metadata to match this version
  --probe                         Probe referenced URLs without downloading full assets
  --download-and-hash             Download assets and verify SHA-256
  --require-cache-headers         Fail probes when cache-control is absent
  --timeout-ms <ms>               Per-request timeout. Default: 15000
  --allow-test-hosts              Permit localhost and .test hosts for fixtures only
  --skip-release-index            Do not validate release-index.json
  --skip-updater-metadata         Do not validate latest.json
  --skip-updater-artifacts        Do not use local updater artifacts; requires --download-and-hash
  --skip-core-manifest            Do not validate core-assets.json`);
}

function isHttpSource(source) {
  return /^https?:\/\//i.test(String(source ?? ""));
}

function displaySource(source) {
  if (isHttpSource(source)) {
    return source;
  }
  const resolved = resolve(repoRoot, source);
  return relative(repoRoot, resolved).replaceAll("\\", "/") || ".";
}

async function readJsonSource(source, label, options) {
  if (!source) {
    return null;
  }

  let text;
  if (isHttpSource(source)) {
    const response = await fetchWithTimeout(source, { method: "GET" }, options.timeoutMs);
    if (!response.ok) {
      throw new Error(`${label} could not be fetched: ${response.status} ${response.statusText}`);
    }
    text = await response.text();
  } else {
    text = await readFile(resolve(repoRoot, source), "utf8");
  }

  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`, { cause: error });
  }
}

function forbiddenHostReason(hostname, options = {}) {
  const host = hostname.toLowerCase();
  if (options.allowTestHosts && (host === "localhost" || host === "127.0.0.1" || host === "::1" || host.endsWith(".test"))) {
    return null;
  }
  if (host === "example.com" || host.endsWith(".example.com") || host.endsWith(".example") || host.includes("example")) {
    return "example host";
  }
  if (
    host === "github.com" ||
    host.endsWith(".github.com") ||
    host === "githubusercontent.com" ||
    host.endsWith(".githubusercontent.com") ||
    host === "github.io" ||
    host.endsWith(".github.io")
  ) {
    return "GitHub host";
  }
  if (host === "localhost" || host === "127.0.0.1" || host === "::1" || host.endsWith(".test")) {
    return "local or test host";
  }
  if (host.includes("placeholder")) {
    return "placeholder host";
  }
  return null;
}

function normalizeBaseUrl(value, label, options = {}) {
  const text = String(value ?? "").trim();
  if (!text) {
    return null;
  }

  let parsed;
  try {
    parsed = new URL(text);
  } catch {
    throw new Error(`${label} is not a valid URL: ${text}`);
  }

  if (parsed.protocol !== "https:") {
    throw new Error(`${label} must use https: ${text}`);
  }
  if (parsed.username || parsed.password) {
    throw new Error(`${label} must not include credentials: ${text}`);
  }

  const reason = forbiddenHostReason(parsed.hostname, options);
  if (reason) {
    throw new Error(`${label} must not use ${reason}: ${text}`);
  }

  parsed.hash = "";
  parsed.search = "";
  return parsed.toString().replace(/\/+$/g, "");
}

function assertAllowedUrl(urlText, label, expectedBaseUrl, options, failures) {
  const text = String(urlText ?? "").trim();
  if (!text) {
    failures.push(`${label} is missing url`);
    return null;
  }

  let parsed;
  try {
    parsed = new URL(text);
  } catch {
    failures.push(`${label} is not a valid URL: ${text}`);
    return null;
  }

  if (parsed.protocol !== "https:") {
    failures.push(`${label} must use https: ${text}`);
  }
  if (parsed.username || parsed.password || parsed.search || parsed.hash) {
    failures.push(`${label} must not include credentials, query strings, or fragments: ${text}`);
  }

  const reason = forbiddenHostReason(parsed.hostname, options);
  if (reason) {
    failures.push(`${label} must not use ${reason}: ${text}`);
  }

  if (expectedBaseUrl && !text.startsWith(`${expectedBaseUrl}/`)) {
    failures.push(`${label} must be derived from ${expectedBaseUrl}: ${text}`);
  }

  return parsed;
}

function requiredString(value, label, failures) {
  if (typeof value !== "string" || value.trim().length === 0) {
    failures.push(`${label} is required`);
    return null;
  }
  return value.trim();
}

function requiredSha256(value, label, failures) {
  const text = requiredString(value, label, failures);
  if (!text) {
    return null;
  }
  if (!/^[a-f0-9]{64}$/i.test(text)) {
    failures.push(`${label} must be a 64-character SHA-256 hex string`);
    return null;
  }
  return text.toLowerCase();
}

function requiredBytes(value, label, failures) {
  if (!Number.isInteger(value) || value <= 0) {
    failures.push(`${label} must be a positive integer byte size`);
    return null;
  }
  return value;
}

function releaseTargetFor(target, arch) {
  const normalizedTarget = String(target ?? "").toLowerCase();
  const normalizedArch = String(arch ?? "").toLowerCase();
  const match = targetMatrix.find((entry) => entry.target === normalizedTarget && entry.arch === normalizedArch);
  return match?.releaseTarget ?? null;
}

function releaseTargetForArtifact(artifact) {
  const explicit = String(artifact.releaseTarget ?? artifact.target ?? "").toLowerCase();
  if (stableTargetSet.has(explicit)) {
    return explicit;
  }
  return releaseTargetFor(artifact.target, artifact.arch);
}

async function walkManifests(root) {
  let entries;
  try {
    entries = await readdir(root, { withFileTypes: true });
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return [];
    }
    throw error;
  }

  const manifests = [];
  for (const entry of entries.sort((left, right) => left.name.localeCompare(right.name))) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      manifests.push(...(await walkManifests(path)));
    } else if (entry.isFile() && entry.name === "artifact-manifest.json") {
      manifests.push(path);
    }
  }
  return manifests.sort((left, right) => left.localeCompare(right));
}

function artifactPath(artifact, context) {
  const value = artifact?.path ?? artifact?.name;
  if (!value || typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${context} is missing path or name`);
  }

  const normalized = value.trim().replaceAll("\\", "/");
  if (normalized.startsWith("/") || normalized.split("/").some((segment) => segment === "..")) {
    throw new Error(`${context} has an unsafe artifact path: ${value}`);
  }

  return normalized;
}

function isUpdaterPayload(artifact) {
  return artifact.kind === "updater" && !String(artifact.name ?? "").toLowerCase().endsWith(".sig");
}

function findSignatureArtifact(payload, artifacts) {
  return artifacts.find((artifact) => {
    if (artifact.kind !== "signature") {
      return false;
    }

    return (
      artifact.originalRelativePath === `${payload.originalRelativePath}.sig` ||
      artifact.originalName === `${payload.originalName}.sig` ||
      artifact.path === `${payload.path}.sig` ||
      artifact.name === `${payload.name}.sig`
    );
  });
}

async function loadUpdaterArtifactTargets(root) {
  const targets = new Map();
  for (const manifestPath of await walkManifests(root)) {
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    const target = String(manifest.target ?? "").trim();
    if (!target) {
      throw new Error(`${relative(repoRoot, manifestPath)} is missing target`);
    }
    if (!Array.isArray(manifest.artifacts)) {
      throw new Error(`${relative(repoRoot, manifestPath)} is missing artifacts[]`);
    }
    if (targets.has(target)) {
      throw new Error(`signed updater artifacts contain multiple manifests for ${target}`);
    }
    targets.set(target, {
      manifestDir: dirname(manifestPath),
      artifacts: manifest.artifacts,
    });
  }
  return targets;
}

function updaterFileNameFromUrl(urlText) {
  const parsed = new URL(urlText);
  const fileName = basename(parsed.pathname);
  try {
    return decodeURIComponent(fileName);
  } catch {
    return fileName;
  }
}

async function assertFile(path, label) {
  let fileStat;
  try {
    fileStat = await stat(path);
  } catch (error) {
    if (error && error.code === "ENOENT") {
      throw new Error(`${label} file is missing: ${path}`, { cause: error });
    }
    throw error;
  }
  if (!fileStat.isFile()) {
    throw new Error(`${label} is not a file: ${path}`);
  }
}

async function verifyUpdaterMetadataSignatures(latest, rawOptions = {}) {
  const env = rawOptions.env ?? process.env;
  const updaterPublicKey = rawOptions.updaterPublicKey ?? resolveApprovedUpdaterPublicKey(env);
  const artifactRoot = rawOptions.updaterArtifacts ? resolve(repoRoot, rawOptions.updaterArtifacts) : null;
  const failures = [];
  const verifications = {};

  if (!artifactRoot) {
    throw new StagingValidationError("updater signatures", [
      "local signed updater artifacts are required for offline signature verification",
    ]);
  }

  let targets;
  try {
    targets = await loadUpdaterArtifactTargets(artifactRoot);
  } catch (error) {
    throw new StagingValidationError("updater signatures", [error.message]);
  }

  for (const target of stableTargets) {
    const platform = latest.platforms?.[target];
    if (!platform || typeof platform !== "object") {
      continue;
    }

    const targetArtifacts = targets.get(target);
    const label = `latest.json platforms.${target}`;
    if (!targetArtifacts) {
      failures.push(`${label} has no local signed updater artifact manifest`);
      continue;
    }

    const expectedPayloadName = updaterFileNameFromUrl(platform.url);
    const payload =
      targetArtifacts.artifacts.find((artifact) => isUpdaterPayload(artifact) && artifact.name === expectedPayloadName) ??
      targetArtifacts.artifacts.find(isUpdaterPayload);
    if (!payload) {
      failures.push(`${label} has no local updater payload artifact`);
      continue;
    }
    if (payload.name !== expectedPayloadName) {
      failures.push(`${label} URL filename ${expectedPayloadName} does not match local payload ${payload.name}`);
      continue;
    }

    const signatureArtifact = findSignatureArtifact(payload, targetArtifacts.artifacts);
    if (!signatureArtifact) {
      failures.push(`${label} has no local .sig artifact for ${payload.name}`);
      continue;
    }

    try {
      const payloadPath = resolve(targetArtifacts.manifestDir, artifactPath(payload, `${label} payload`));
      const signaturePath = resolve(targetArtifacts.manifestDir, artifactPath(signatureArtifact, `${label} signature`));
      await assertFile(payloadPath, `${label} payload`);
      await assertFile(signaturePath, `${label} signature`);
      const signature = (await readFile(signaturePath, "utf8")).trim();
      if (signature !== String(platform.signature ?? "").trim()) {
        failures.push(`${label} signature does not match local .sig artifact`);
        continue;
      }
      verifications[target] = await verifyTauriUpdaterSignatureFile(
        payloadPath,
        signature,
        updaterPublicKey,
        `${label} payload`,
      );
    } catch (error) {
      failures.push(error.message);
    }
  }

  assertNoFailures("updater signatures", failures);
  return {
    verifiedCount: Object.keys(verifications).length,
    targets: Object.keys(verifications).sort((left, right) => left.localeCompare(right)),
    verifications,
  };
}

function assertStableMatrix(label, present, failures) {
  const missing = stableTargets.filter((target) => !present.has(target));
  if (missing.length > 0) {
    failures.push(`${label} is missing stable targets: ${missing.join(", ")}`);
  }
}

function assertNoFailures(label, failures) {
  if (failures.length > 0) {
    throw new StagingValidationError(label, failures);
  }
}

function validationOptions(options) {
  return {
    allowTestHosts: Boolean(options.allowTestHosts),
    expectedVersion: options.expectedVersion ?? null,
    cdnBaseUrl: options.cdnBaseUrl ?? null,
    updatesBaseUrl: options.updatesBaseUrl ?? null,
  };
}

function validateReleaseIndex(index, rawOptions = {}) {
  const options = validationOptions(rawOptions);
  const failures = [];
  const baseUrl = normalizeBaseUrl(options.cdnBaseUrl ?? index?.baseUrl, "CDN base URL", options);
  if (!baseUrl) {
    failures.push("CDN base URL is required for release-index validation");
  }

  if (!index || typeof index !== "object") {
    throw new StagingValidationError("release index", ["release-index JSON must be an object"]);
  }
  if (index.channel !== "stable") {
    failures.push(`release-index channel must be stable, got ${String(index.channel)}`);
  }
  if (options.expectedVersion && index.version !== options.expectedVersion) {
    failures.push(`release-index version must be ${options.expectedVersion}, got ${String(index.version)}`);
  }
  if (!Array.isArray(index.artifacts) || index.artifacts.length === 0) {
    failures.push("release-index artifacts must be a non-empty array");
  }

  const presentTargets = new Set();
  const candidates = [];
  for (const [artifactIndex, artifact] of (index.artifacts ?? []).entries()) {
    const label = `release-index artifacts[${artifactIndex}]`;
    const releaseTarget = releaseTargetForArtifact(artifact);
    if (!releaseTarget) {
      failures.push(`${label} must identify a stable target`);
    } else {
      presentTargets.add(releaseTarget);
    }

    if (artifact.channel !== "stable") {
      failures.push(`${label} channel must be stable`);
    }
    if (options.expectedVersion && artifact.version !== options.expectedVersion) {
      failures.push(`${label} version must be ${options.expectedVersion}`);
    }
    requiredString(artifact.name, `${label} name`, failures);
    requiredString(artifact.kind, `${label} kind`, failures);
    const bytes = requiredBytes(artifact.bytes, `${label} bytes`, failures);
    const sha256 = requiredSha256(artifact.sha256, `${label} sha256`, failures);
    const parsed = assertAllowedUrl(artifact.url, `${label} url`, baseUrl, options, failures);
    if (parsed && bytes && sha256) {
      candidates.push({ label, url: parsed.toString(), bytes, sha256 });
    }
  }
  assertStableMatrix("release-index", presentTargets, failures);
  assertNoFailures("release index", failures);

  return {
    artifactCount: index.artifacts.length,
    targets: stableTargets.filter((target) => presentTargets.has(target)),
    candidates,
  };
}

function placeholderSignature(signature) {
  return /placeholder|replace_before_release|replace-before-release|changeme|\btodo\b|\btbd\b|voyavpn\.example/i.test(
    String(signature ?? ""),
  );
}

function validateUpdaterMetadata(latest, rawOptions = {}) {
  const options = validationOptions(rawOptions);
  const failures = [];
  const baseUrl = normalizeBaseUrl(options.updatesBaseUrl, "updater base URL", options);
  if (!baseUrl) {
    failures.push("updater base URL is required for latest.json validation");
  }

  if (!latest || typeof latest !== "object") {
    throw new StagingValidationError("updater metadata", ["latest.json must be an object"]);
  }
  if (options.expectedVersion && latest.version !== options.expectedVersion) {
    failures.push(`latest.json version must be ${options.expectedVersion}, got ${String(latest.version)}`);
  }
  if (!latest.platforms || typeof latest.platforms !== "object" || Array.isArray(latest.platforms)) {
    failures.push("latest.json platforms must be an object");
  }

  const presentTargets = new Set(Object.keys(latest.platforms ?? {}));
  assertStableMatrix("latest.json", presentTargets, failures);

  const candidates = [];
  for (const target of stableTargets) {
    const platform = latest.platforms?.[target];
    const label = `latest.json platforms.${target}`;
    if (!platform || typeof platform !== "object") {
      failures.push(`${label} is required`);
      continue;
    }
    const signature = requiredString(platform.signature, `${label} signature`, failures);
    if (signature && placeholderSignature(signature)) {
      failures.push(`${label} signature must be a real non-placeholder Tauri updater signature`);
    }
    const parsed = assertAllowedUrl(platform.url, `${label} url`, baseUrl, options, failures);
    if (parsed) {
      candidates.push({ label, url: parsed.toString(), bytes: null, sha256: null, updaterSignature: signature, updaterTarget: target });
    }
  }
  assertNoFailures("updater metadata", failures);

  return {
    platformCount: Object.keys(latest.platforms).length,
    targets: stableTargets.filter((target) => presentTargets.has(target)),
    candidates,
  };
}

function validateCoreManifest(manifest, rawOptions = {}) {
  const options = validationOptions(rawOptions);
  const failures = [];
  const baseUrl = normalizeBaseUrl(options.cdnBaseUrl ?? manifest?.baseUrl, "CDN base URL", options);
  if (!baseUrl) {
    failures.push("CDN base URL is required for core manifest validation");
  }

  if (!manifest || typeof manifest !== "object") {
    throw new StagingValidationError("core manifest", ["core manifest JSON must be an object"]);
  }
  if (manifest.channel !== "stable") {
    failures.push(`core manifest channel must be stable, got ${String(manifest.channel)}`);
  }
  if (!Array.isArray(manifest.assets) || manifest.assets.length === 0) {
    failures.push("core manifest assets must be a non-empty array");
  }

  const seen = new Set();
  const candidates = [];
  for (const [assetIndex, asset] of (manifest.assets ?? []).entries()) {
    const label = `core manifest assets[${assetIndex}]`;
    const coreType = requiredString(asset.coreType, `${label} coreType`, failures);
    const os = requiredString(asset.os, `${label} os`, failures);
    const arch = requiredString(asset.arch, `${label} arch`, failures);
    if (coreType && !stableCoreTypes.includes(coreType)) {
      failures.push(`${label} coreType must be one of ${stableCoreTypes.join(", ")}`);
    }
    if (os && !stableOs.includes(os)) {
      failures.push(`${label} os must be one of ${stableOs.join(", ")}`);
    }
    if (arch && !stableArchs.includes(arch)) {
      failures.push(`${label} arch must be one of ${stableArchs.join(", ")}`);
    }
    if (coreType && os && arch) {
      const key = `${coreType}/${os}/${arch}`;
      if (seen.has(key)) {
        failures.push(`core manifest has duplicate entry for ${key}`);
      }
      seen.add(key);
    }

    requiredString(asset.version, `${label} version`, failures);
    requiredString(asset.license, `${label} license`, failures);
    requiredString(asset.name, `${label} name`, failures);
    const bytes = requiredBytes(asset.bytes, `${label} bytes`, failures);
    const sha256 = requiredSha256(asset.sha256, `${label} sha256`, failures);
    const parsed = assertAllowedUrl(asset.url, `${label} url`, baseUrl, options, failures);
    if (asset.upstreamUrl) {
      try {
        new URL(asset.upstreamUrl);
      } catch {
        failures.push(`${label} upstreamUrl is not a valid URL`);
      }
    }
    if (parsed && bytes && sha256) {
      candidates.push({ label, url: parsed.toString(), bytes, sha256 });
    }
  }

  const missing = [];
  for (const coreType of stableCoreTypes) {
    for (const os of stableOs) {
      for (const arch of stableArchs) {
        const key = `${coreType}/${os}/${arch}`;
        if (!seen.has(key)) {
          missing.push(key);
        }
      }
    }
  }
  if (missing.length > 0) {
    failures.push(`core manifest is missing required entries: ${missing.join(", ")}`);
  }
  assertNoFailures("core manifest", failures);

  return {
    assetCount: manifest.assets.length,
    coreTypes: stableCoreTypes.filter((coreType) => manifest.assets.some((asset) => asset.coreType === coreType)),
    candidates,
  };
}

async function fetchWithTimeout(url, init, timeoutMs) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetch(url, { ...init, signal: controller.signal });
  } finally {
    clearTimeout(timer);
  }
}

async function probeCandidate(candidate, options) {
  if (options.downloadAndHash && candidate.sha256) {
    return downloadAndHashCandidate(candidate, options);
  }

  let response = await fetchWithTimeout(candidate.url, { method: "HEAD" }, options.timeoutMs);
  if (response.status === 405 || response.status === 403) {
    response = await fetchWithTimeout(candidate.url, { headers: { range: "bytes=0-0" }, method: "GET" }, options.timeoutMs);
  }
  if (!response.ok && response.status !== 206) {
    throw new Error(`${candidate.label} probe failed: ${response.status} ${response.statusText}`);
  }

  const contentLength = response.headers.get("content-length");
  if (candidate.bytes && contentLength && Number.parseInt(contentLength, 10) !== candidate.bytes && response.status !== 206) {
    throw new Error(`${candidate.label} content-length mismatch: expected ${candidate.bytes}, got ${contentLength}`);
  }

  const cacheControl = response.headers.get("cache-control");
  if (options.requireCacheHeaders && !cacheControl) {
    throw new Error(`${candidate.label} is missing cache-control header`);
  }

  return {
    label: candidate.label,
    status: response.status,
    cacheControl,
    checked: "probe",
  };
}

async function downloadAndHashCandidate(candidate, options) {
  const response = await fetchWithTimeout(candidate.url, { method: "GET" }, options.timeoutMs);
  if (!response.ok) {
    throw new Error(`${candidate.label} download failed: ${response.status} ${response.statusText}`);
  }

  const hash = createHash("sha256");
  let bytes = 0;
  const signatureChunks = candidate.updaterSignature ? [] : null;
  for await (const chunk of response.body) {
    const buffer = Buffer.from(chunk);
    hash.update(buffer);
    if (signatureChunks) {
      signatureChunks.push(buffer);
    }
    bytes += buffer.length;
  }

  const actualSha256 = hash.digest("hex");
  if (candidate.bytes && bytes !== candidate.bytes) {
    throw new Error(`${candidate.label} byte mismatch: expected ${candidate.bytes}, got ${bytes}`);
  }
  if (candidate.sha256 && actualSha256 !== candidate.sha256.toLowerCase()) {
    throw new Error(`${candidate.label} sha256 mismatch: expected ${candidate.sha256}, got ${actualSha256}`);
  }
  let updaterSignature = null;
  if (signatureChunks) {
    updaterSignature = verifyTauriUpdaterSignature(
      Buffer.concat(signatureChunks),
      candidate.updaterSignature,
      options.updaterPublicKey,
      `${candidate.label} payload`,
    );
  }

  const cacheControl = response.headers.get("cache-control");
  if (options.requireCacheHeaders && !cacheControl) {
    throw new Error(`${candidate.label} is missing cache-control header`);
  }

  return {
    label: candidate.label,
    status: response.status,
    cacheControl,
    bytes,
    sha256: actualSha256,
    updaterSignature,
    checked: "download-and-hash",
  };
}

async function probeCandidates(candidates, options) {
  const failures = [];
  const results = [];
  for (const candidate of candidates) {
    try {
      results.push(await probeCandidate(candidate, options));
    } catch (error) {
      failures.push(error.message);
    }
  }
  assertNoFailures("CDN artifact probe", failures);
  return results;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const allCandidates = [];
  const summaries = [];

  if (options.releaseIndex) {
    const releaseIndex = await readJsonSource(options.releaseIndex, "release index", options);
    const summary = validateReleaseIndex(releaseIndex, options);
    allCandidates.push(...summary.candidates);
    summaries.push(`release-index: ${summary.artifactCount} artifacts across ${summary.targets.length} stable targets`);
  }

  if (options.updaterMetadata) {
    options.updaterPublicKey = resolveApprovedUpdaterPublicKey();
    const latest = await readJsonSource(options.updaterMetadata, "updater metadata", options);
    const summary = validateUpdaterMetadata(latest, options);
    let signatureSummary = null;
    if (options.updaterArtifacts) {
      signatureSummary = await verifyUpdaterMetadataSignatures(latest, options);
    } else if (!options.downloadAndHash) {
      throw new Error("latest.json signature verification requires --updater-artifacts or --download-and-hash");
    }
    allCandidates.push(...summary.candidates);
    summaries.push(
      signatureSummary
        ? `latest.json: ${summary.platformCount} updater platforms; ${signatureSummary.verifiedCount} signatures verified`
        : `latest.json: ${summary.platformCount} updater platforms; signatures verify during download-and-hash`,
    );
  }

  if (options.coreManifest) {
    const coreManifest = await readJsonSource(options.coreManifest, "core manifest", options);
    const summary = validateCoreManifest(coreManifest, options);
    allCandidates.push(...summary.candidates);
    summaries.push(`core manifest: ${summary.assetCount} assets for ${summary.coreTypes.join(", ")}`);
  }

  if (summaries.length === 0) {
    throw new Error("At least one metadata input must be validated");
  }

  console.log("VoyaVPN release staging verification");
  console.log(`release-index: ${options.releaseIndex ? displaySource(options.releaseIndex) : "skipped"}`);
  console.log(`latest.json: ${options.updaterMetadata ? displaySource(options.updaterMetadata) : "skipped"}`);
  console.log(`core manifest: ${options.coreManifest ? displaySource(options.coreManifest) : "skipped"}`);
  for (const summary of summaries) {
    console.log(`[PASS] ${summary}`);
  }

  if (options.probe) {
    const probeResults = await probeCandidates(allCandidates, options);
    console.log(`[PASS] CDN ${options.downloadAndHash ? "download/hash" : "probe"} checked ${probeResults.length} URLs`);
  } else {
    console.log("[SKIP] CDN network probe not requested");
  }
}

function isCliEntrypoint() {
  return process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href === import.meta.url : false;
}

if (isCliEntrypoint()) {
  main().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}

export {
  StagingValidationError,
  forbiddenHostReason,
  normalizeBaseUrl,
  validateCoreManifest,
  validateReleaseIndex,
  validateUpdaterMetadata,
  verifyUpdaterMetadataSignatures,
};
