import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { resolveApprovedUpdaterPublicKey, verifyTauriUpdaterSignatureFile } from "./updater-signatures.mjs";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const stableChannel = "stable";
const stableTargets = [
  "darwin-aarch64",
  "darwin-x86_64",
  "linux-aarch64",
  "linux-x86_64",
  "windows-aarch64",
  "windows-x86_64",
];
const stableTargetSet = new Set(stableTargets);

function parseArgs(argv) {
  const options = {
    input: "dist/release",
    output: "dist/release/latest.json",
    evidenceOutput: null,
    version: null,
    channel: "beta",
    baseUrl: null,
    notes: null,
    pubDate: null,
    placeholderSignatures: false,
    targets: [],
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
      case "--input":
        options.input = next();
        break;
      case "--output":
      case "--out":
        options.output = next();
        break;
      case "--evidence-out":
        options.evidenceOutput = next();
        break;
      case "--version":
        options.version = next();
        break;
      case "--channel":
        options.channel = next();
        break;
      case "--base-url":
        options.baseUrl = next();
        break;
      case "--notes":
        options.notes = next();
        break;
      case "--pub-date":
        options.pubDate = next();
        break;
      case "--placeholder-signatures":
        options.placeholderSignatures = true;
        break;
      case "--target":
        options.targets.push(...next().split(",").map((target) => target.trim()).filter(Boolean));
        break;
      case "--help":
        printHelp();
        process.exit(0);
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return options;
}

function printHelp() {
  console.log(`Usage: node scripts/release-updater-metadata.mjs [options]

Options:
  --input <dir>                Directory containing artifact-manifest.json files. Default: dist/release
  --out <file>                 latest.json output path. Default: dist/release/latest.json
  --evidence-out <file>        Evidence JSON output path. Default: sibling *.evidence.json
  --version <semver>           App version. Defaults to package.json version
  --channel <name>             Release channel. Default: beta
  --base-url <url>             Public update asset base URL. Defaults to VOYAVPN_UPDATES_BASE_URL,
                               then https://cdn.voyavpn.test/<channel>/updater for non-stable
  --notes <text>               Release notes string
  --pub-date <iso>             Publication timestamp. Default: current time
  --target <platform[,..]>     Platform key to include when no manifest exists
  --placeholder-signatures     Emit dry-run placeholder updater URLs and signatures.
                               Stable rejects this option and requires signed payloads.`);
}

async function readPackageVersion() {
  const packageJson = JSON.parse(await readFile(resolve(repoRoot, "package.json"), "utf8"));
  return packageJson.version;
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

function isStable(channel) {
  return channel.trim().toLowerCase() === stableChannel;
}

function isForbiddenStableHost(hostname) {
  const host = hostname.toLowerCase();
  return (
    host === "example.com" ||
    host.endsWith(".example.com") ||
    host.endsWith(".example") ||
    host.includes("example") ||
    host === "github.com" ||
    host.endsWith(".github.com") ||
    host === "githubusercontent.com" ||
    host.endsWith(".githubusercontent.com") ||
    host === "github.io" ||
    host.endsWith(".github.io")
  );
}

function normalizeBaseUrl(baseUrl, channel) {
  const value = (baseUrl ?? "").trim();
  if (!value) {
    throw new Error(
      isStable(channel)
        ? "Stable updater metadata requires --base-url or VOYAVPN_UPDATES_BASE_URL"
        : "Updater metadata generation requires --base-url or VOYAVPN_UPDATES_BASE_URL",
    );
  }

  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    throw new Error(`Invalid updater base URL: ${value}`);
  }

  if (isStable(channel) && parsed.protocol !== "https:") {
    throw new Error(`Stable updater base URL must use https: ${value}`);
  }
  if (!isStable(channel) && parsed.protocol !== "https:" && parsed.protocol !== "http:") {
    throw new Error(`Updater base URL must use http or https: ${value}`);
  }

  parsed.hash = "";
  parsed.search = "";

  const normalized = parsed.toString().replace(/\/+$/g, "");
  const stableUrlText = normalized.toLowerCase();
  if (isStable(channel) && (isForbiddenStableHost(parsed.hostname) || stableUrlText.includes("placeholder"))) {
    throw new Error(`Stable updater base URL must not use example, GitHub, or placeholder hosts: ${value}`);
  }

  return normalized;
}

function resolveBaseUrl(options) {
  const configuredBaseUrl = options.baseUrl ?? process.env.VOYAVPN_UPDATES_BASE_URL;
  if (configuredBaseUrl !== undefined && configuredBaseUrl !== null) {
    return normalizeBaseUrl(configuredBaseUrl, options.channel);
  }

  if (isStable(options.channel)) {
    return normalizeBaseUrl(null, options.channel);
  }

  return normalizeBaseUrl(`https://cdn.voyavpn.test/${options.channel}/updater`, options.channel);
}

function joinUrl(baseUrl, ...parts) {
  const cleanBase = baseUrl.replace(/\/+$/g, "");
  const cleanParts = parts.map((part) => encodeURIComponent(part).replaceAll("%2F", "/"));
  return [cleanBase, ...cleanParts].join("/");
}

function defaultEvidencePath(outputPath) {
  const name = basename(outputPath);
  const dot = name.lastIndexOf(".");
  const evidenceName = dot === -1 ? `${name}.evidence.json` : `${name.slice(0, dot)}.evidence.json`;
  return join(dirname(outputPath), evidenceName);
}

function displayRepoPath(path) {
  return relative(repoRoot, path).replaceAll("\\", "/") || ".";
}

function sourceInputEvidence(inputPath) {
  const relativePath = displayRepoPath(inputPath);
  const isFixture = relativePath === "tests/fixtures" || relativePath.startsWith("tests/fixtures/");
  return {
    path: inputPath,
    relativePath,
    kind: isFixture ? "fixture" : "workflow-artifact",
    nonPublishableFixture: isFixture,
  };
}

function uniqueSorted(values) {
  return [...new Set(values.filter(Boolean))].sort((left, right) => left.localeCompare(right));
}

function describeStableTarget(target) {
  const details = {
    "darwin-aarch64": { os: "macos", arch: "arm64" },
    "darwin-x86_64": { os: "macos", arch: "x64" },
    "linux-aarch64": { os: "linux", arch: "arm64" },
    "linux-x86_64": { os: "linux", arch: "x64" },
    "windows-aarch64": { os: "windows", arch: "arm64" },
    "windows-x86_64": { os: "windows", arch: "x64" },
  }[target];

  return {
    target,
    os: details?.os ?? null,
    arch: details?.arch ?? null,
  };
}

function placeholderToken(target) {
  return `VOYAVPN_UPDATER_SIGNATURE_PLACEHOLDER_${target.toUpperCase().replace(/[^A-Z0-9]+/g, "_")}`;
}

function isUpdaterPayload(artifact) {
  return artifact.kind === "updater" && !artifact.name.toLowerCase().endsWith(".sig");
}

function findSignatureArtifact(payload, artifacts) {
  return artifacts.find((artifact) => {
    if (artifact.kind !== "signature") {
      return false;
    }

    return (
      artifact.originalRelativePath === `${payload.originalRelativePath}.sig` ||
      artifact.originalName === `${payload.originalName}.sig`
    );
  });
}

function artifactPath(artifact, context) {
  const value = artifact.path ?? artifact.name;
  if (!value || typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${context} is missing path or name`);
  }

  const normalized = value.trim().replaceAll("\\", "/");
  if (normalized.startsWith("/") || normalized.split("/").some((segment) => segment === "..")) {
    throw new Error(`${context} has an unsafe artifact path: ${value}`);
  }

  return normalized;
}

function requiredString(value, field, context) {
  if (!value || typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${context} is missing ${field}`);
  }
  return value.trim();
}

function requiredSha256(value, context) {
  if (!value || typeof value !== "string" || !/^[a-fA-F0-9]{64}$/.test(value)) {
    throw new Error(`${context} is missing a valid sha256`);
  }
  return value.toLowerCase();
}

function requiredBytes(value, context) {
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${context} is missing valid bytes`);
  }
  return value;
}

function isPlaceholderSignature(signature) {
  const value = signature.trim().toLowerCase();
  return (
    value.length === 0 ||
    value.includes("placeholder") ||
    value.includes("replace_before_release") ||
    value.includes("replace-before-release") ||
    value === "todo" ||
    value === "tbd" ||
    value === "changeme"
  );
}

function assertStableSignature(signature, target) {
  if (isPlaceholderSignature(signature)) {
    throw new Error(`Stable updater signature for ${target} is a placeholder or empty value`);
  }
}

async function sha256(path) {
  const hash = createHash("sha256");
  await new Promise((resolvePromise, rejectPromise) => {
    const stream = createReadStream(path);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("error", rejectPromise);
    stream.on("end", resolvePromise);
  });
  return hash.digest("hex");
}

async function verifyArtifactFile(manifestDir, artifact, context, requireManifestMetadata) {
  const path = artifactPath(artifact, context);
  const fullPath = resolve(manifestDir, path);
  let fileStat;
  try {
    fileStat = await stat(fullPath);
  } catch (error) {
    if (error && error.code === "ENOENT") {
      throw new Error(`${context} file is missing: ${path}`, { cause: error });
    }
    throw error;
  }

  if (!fileStat.isFile()) {
    throw new Error(`${context} is not a file: ${path}`);
  }

  const actualBytes = fileStat.size;
  const actualSha256 = await sha256(fullPath);

  if (requireManifestMetadata) {
    const expectedBytes = requiredBytes(artifact.bytes, context);
    const expectedSha256 = requiredSha256(artifact.sha256, context);
    if (actualBytes !== expectedBytes) {
      throw new Error(`${context} bytes do not match manifest: expected ${expectedBytes}, got ${actualBytes}`);
    }
    if (actualSha256 !== expectedSha256) {
      throw new Error(`${context} sha256 does not match manifest: expected ${expectedSha256}, got ${actualSha256}`);
    }
  }

  return {
    path,
    bytes: actualBytes,
    sha256: actualSha256,
  };
}

function assertStableArtifactMetadata(artifact, target, version, channel, context) {
  requiredString(artifact.name, "name", context);
  requiredString(artifact.originalName, "originalName", context);
  artifactPath(artifact, context);

  if (requiredString(artifact.target, "target", context) !== target) {
    throw new Error(`${context} target does not match manifest target ${target}`);
  }
  if (requiredString(artifact.channel, "channel", context) !== channel) {
    throw new Error(`${context} channel does not match requested channel ${channel}`);
  }
  if (requiredString(artifact.version, "version", context) !== version) {
    throw new Error(`${context} version does not match requested version ${version}`);
  }
}

function assertStableTargetNames(platformKeys) {
  const keys = [...platformKeys].sort((left, right) => left.localeCompare(right));
  const unsupported = keys.filter((target) => !stableTargetSet.has(target));
  if (unsupported.length > 0) {
    throw new Error(`Stable updater metadata contains unsupported target(s): ${unsupported.join(", ")}`);
  }
}

function assertStableTargetMatrix(platformKeys) {
  const keys = [...platformKeys].sort((left, right) => left.localeCompare(right));
  const missing = stableTargets.filter((target) => !keys.includes(target));
  if (missing.length > 0) {
    throw new Error(`Stable updater metadata is missing signed payloads for target(s): ${missing.join(", ")}`);
  }
}

function assertStableDocuments(latest, evidenceDocument, baseUrl) {
  const latestSerialized = JSON.stringify(latest).toLowerCase();
  if (
    latestSerialized.includes("github.com") ||
    latestSerialized.includes("voyavpn.example") ||
    latestSerialized.includes("placeholder")
  ) {
    throw new Error("Stable updater latest.json contains forbidden placeholder or GitHub content");
  }

  for (const [target, platform] of Object.entries(latest.platforms)) {
    if (!platform.url.startsWith(`${baseUrl}/`)) {
      throw new Error(`Stable updater URL for ${target} is not derived from base URL: ${platform.url}`);
    }
    const evidence = evidenceDocument.platforms[target];
    if (evidence?.source !== "signed-artifact") {
      throw new Error(`Stable updater evidence for ${target} does not map to a signed artifact`);
    }
    if (evidence.signatureVerified !== true) {
      throw new Error(`Stable updater signature for ${target} was not verified with the approved updater public key`);
    }
  }
}

function buildTargetEvidence(evidence) {
  return evidence.map((entry) => {
    const described = describeStableTarget(entry.target);
    const artifactNames = [entry.artifact, entry.signatureArtifact].filter(Boolean);
    const sourceArtifactNames = [entry.sourceArtifactName, entry.sourceSignatureArtifactName].filter(Boolean);
    const checksums = [
      entry.sha256
        ? {
            name: entry.artifact,
            sourceArtifactName: entry.sourceArtifactName,
            bytes: entry.bytes,
            sha256: entry.sha256,
          }
        : null,
      entry.signatureSha256
        ? {
            name: entry.signatureArtifact,
            sourceArtifactName: entry.sourceSignatureArtifactName,
            bytes: entry.signatureBytes,
            sha256: entry.signatureSha256,
          }
        : null,
    ].filter(Boolean);

    return {
      ...described,
      source: entry.source,
      artifactCount: artifactNames.length,
      artifactNames,
      sourceArtifactNames,
      checksums,
    };
  });
}

async function loadManifests(inputDir) {
  const manifests = [];
  for (const manifestPath of await walkManifests(inputDir)) {
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    manifests.push({
      manifestPath,
      manifestDir: dirname(manifestPath),
      manifest,
    });
  }
  return manifests;
}

async function readSignature(manifestDir, signatureArtifact) {
  const signaturePath = resolve(manifestDir, artifactPath(signatureArtifact, "signature artifact"));
  return (await readFile(signaturePath, "utf8")).trim();
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (isStable(options.channel) && options.placeholderSignatures) {
    throw new Error("Stable updater metadata cannot use --placeholder-signatures; use a dry-run channel for placeholders.");
  }

  const version = options.version ?? (await readPackageVersion());
  const baseUrl = resolveBaseUrl(options);
  const notes = options.notes ?? `VoyaVPN ${version} ${options.channel} release`;
  const pubDate = options.pubDate ?? new Date().toISOString();
  const inputDir = resolve(repoRoot, options.input);
  const outputPath = resolve(repoRoot, options.output);
  const evidencePath = resolve(repoRoot, options.evidenceOutput ?? defaultEvidencePath(outputPath));
  const updaterPublicKey = isStable(options.channel) ? resolveApprovedUpdaterPublicKey() : null;

  const loadedManifests = await loadManifests(inputDir);
  const targets = new Map();

  for (const target of options.targets) {
    targets.set(target, { target, artifacts: [], manifestDir: null });
  }

  for (const loaded of loadedManifests) {
    const manifestLabel = relative(repoRoot, loaded.manifestPath).replaceAll("\\", "/");
    const target = requiredString(loaded.manifest.target, "target", manifestLabel);
    if (!Array.isArray(loaded.manifest.artifacts)) {
      throw new Error(`${manifestLabel} is missing artifacts[]`);
    }
    const current = targets.get(target) ?? { target, artifacts: [], manifestDir: loaded.manifestDir };
    if (isStable(options.channel) && current.manifestDir && current.manifestDir !== loaded.manifestDir) {
      throw new Error(`Stable updater metadata has multiple manifests for target ${target}`);
    }
    current.artifacts.push(...loaded.manifest.artifacts);
    current.manifestDir = loaded.manifestDir;
    targets.set(target, current);
  }

  if (targets.size === 0) {
    throw new Error("No target manifests found. Pass --target with --placeholder-signatures for dry-run metadata.");
  }

  if (isStable(options.channel)) {
    assertStableTargetNames(targets.keys());
  }

  const platforms = {};
  const evidence = [];
  const evidencePlatforms = {};
  const sourceManifests = loadedManifests
    .map((loaded) => relative(repoRoot, loaded.manifestPath).replaceAll("\\", "/"))
    .sort((left, right) => left.localeCompare(right));

  for (const target of [...targets.keys()].sort()) {
    const targetArtifacts = targets.get(target);
    const payload = targetArtifacts.artifacts.find(isUpdaterPayload);
    const signatureArtifact = payload ? findSignatureArtifact(payload, targetArtifacts.artifacts) : null;

    if (payload && signatureArtifact) {
      const requireManifestMetadata = isStable(options.channel);
      if (isStable(options.channel)) {
        assertStableArtifactMetadata(payload, target, version, options.channel, `${target} updater payload`);
        assertStableArtifactMetadata(signatureArtifact, target, version, options.channel, `${target} updater signature`);
      }
      const payloadEvidence = await verifyArtifactFile(
        targetArtifacts.manifestDir,
        payload,
        `${target} updater payload`,
        requireManifestMetadata,
      );
      const signatureEvidence = await verifyArtifactFile(
        targetArtifacts.manifestDir,
        signatureArtifact,
        `${target} updater signature`,
        requireManifestMetadata,
      );
      const signature = await readSignature(targetArtifacts.manifestDir, signatureArtifact);
      let signatureVerification = null;
      if (isStable(options.channel)) {
        assertStableSignature(signature, target);
        signatureVerification = await verifyTauriUpdaterSignatureFile(
          resolve(targetArtifacts.manifestDir, payloadEvidence.path),
          signature,
          updaterPublicKey,
          `${target} updater payload`,
        );
      }

      const url = joinUrl(baseUrl, payload.name);
      platforms[target] = {
        signature,
        url,
      };
      const evidenceEntry = {
        target,
        source: "signed-artifact",
        artifact: payload.name,
        signatureArtifact: signatureArtifact.name,
        channel: options.channel,
        version,
        url,
        bytes: payloadEvidence.bytes,
        sha256: payloadEvidence.sha256,
        signatureBytes: signatureEvidence.bytes,
        signatureSha256: signatureEvidence.sha256,
        sourceArtifactName: payload.originalName ?? payload.name,
        sourceArtifactPath: payload.originalRelativePath,
        sourceSignatureArtifactName: signatureArtifact.originalName ?? signatureArtifact.name,
        sourceSignatureArtifactPath: signatureArtifact.originalRelativePath,
        ...(signatureVerification
          ? {
              signatureVerified: true,
              signatureAlgorithm: signatureVerification.algorithm,
              signatureKeyId: signatureVerification.keyId,
              signaturePrehashed: signatureVerification.prehashed,
              signatureTrustedComment: signatureVerification.trustedComment,
            }
          : {}),
      };
      evidence.push(evidenceEntry);
      evidencePlatforms[target] = evidenceEntry;
      continue;
    }

    if (!options.placeholderSignatures) {
      throw new Error(
        `No signed updater payload found for ${target}. Re-run with --placeholder-signatures only for dry-run metadata.`,
      );
    }

    const placeholderName = `voyavpn-${version}-${options.channel}-${target}-updater.zip`;
    platforms[target] = {
      signature: placeholderToken(target),
      url: joinUrl(baseUrl, version, placeholderName),
    };
    const evidenceEntry = {
      target,
      source: "placeholder",
      artifact: placeholderName,
      channel: options.channel,
      version,
      url: platforms[target].url,
    };
    evidence.push(evidenceEntry);
    evidencePlatforms[target] = evidenceEntry;
  }

  const latest = {
    version,
    notes,
    pub_date: pubDate,
    platforms,
  };

  if (isStable(options.channel)) {
    assertStableTargetMatrix(Object.keys(platforms));
  }

  const generatedAt = new Date().toISOString();
  const targetEvidence = buildTargetEvidence(evidence);
  const firstStableTargets = targetEvidence
    .map((target) => target.target)
    .filter((target) => stableTargetSet.has(target));
  const evidenceDocument = {
    channel: options.channel,
    version,
    baseUrl,
    generatedAt,
    latestPath: outputPath,
    evidencePath,
    sourceInput: sourceInputEvidence(inputDir),
    sourceManifests,
    platformCount: Object.keys(platforms).length,
    targetCount: targetEvidence.length,
    firstStableTargetCount: firstStableTargets.length,
    firstStableTargets,
    checksumCount: evidence.reduce((count, entry) => count + (entry.sha256 ? 1 : 0) + (entry.signatureSha256 ? 1 : 0), 0),
    sourceArtifactNames: uniqueSorted(
      evidence.flatMap((entry) => [entry.sourceArtifactName, entry.sourceSignatureArtifactName]),
    ),
    validations: {
      urlsDerivedFromBaseUrl: true,
      signedArtifactsRequiredForStable: isStable(options.channel),
      stableTargetMatrixComplete: isStable(options.channel),
      updaterPublicKeyApproved: isStable(options.channel),
      updaterSignaturesVerified: isStable(options.channel)
        ? evidence.every((entry) => entry.source === "signed-artifact" && entry.signatureVerified === true)
        : false,
    },
    targets: targetEvidence,
    evidence,
    platforms: evidencePlatforms,
  };

  if (isStable(options.channel)) {
    assertStableDocuments(latest, evidenceDocument, baseUrl);
  }

  await mkdir(dirname(outputPath), { recursive: true });
  await mkdir(dirname(evidencePath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(latest, null, 2)}\n`);
  await writeFile(evidencePath, `${JSON.stringify(evidenceDocument, null, 2)}\n`);

  console.log(`Wrote updater metadata to ${relative(repoRoot, outputPath)}`);
  console.log(`Wrote updater evidence to ${relative(repoRoot, evidencePath)}`);
  const signedEvidence = evidence.filter((entry) => entry.source === "signed-artifact");
  if (signedEvidence.length > 0) {
    console.log(`Signed updater artifacts: ${signedEvidence.map((entry) => `${entry.target}=${entry.artifact}`).join(", ")}`);
    const verifiedEvidence = signedEvidence.filter((entry) => entry.signatureVerified === true);
    if (verifiedEvidence.length > 0) {
      console.log(
        `Verified updater signatures: ${verifiedEvidence.map((entry) => `${entry.target}=${entry.signatureKeyId}`).join(", ")}`,
      );
    }
  } else {
    console.log(`Dry-run updater placeholders: ${evidence.map((entry) => `${entry.target}=${entry.artifact}`).join(", ")}`);
  }
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
