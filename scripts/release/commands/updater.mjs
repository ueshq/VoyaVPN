import { readFile, stat } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";
import { parseArgs } from "../../lib/args.mjs";
import { readPackageVersion, repoRootFromScript } from "../../lib/common.mjs";
import { readJson, sha256File, writeJson } from "../../lib/fs.mjs";
import { resolveApprovedUpdaterPublicKey, verifyTauriUpdaterSignatureFile } from "../updater-signatures.mjs";
import { stableTargets } from "../matrix.mjs";
import {
  artifactEvidence,
  assertStableDocuments,
  assertStableTargetMatrix,
  defaultEvidencePath,
  findSignatureArtifact,
  joinUrl,
  isStableChannel,
  normalizeChannelBaseUrl,
  requiredBytes,
  requiredSha256,
  requiredString,
  safeArtifactPath,
  selectUpdaterPayload,
  sourceInputEvidence,
  uniqueSorted,
  walkArtifactManifests,
} from "../validation.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const stableUpdaterTargets = stableTargets
  .map((target) => target.updater)
  .sort((left, right) => left.localeCompare(right));
const stableUpdaterTargetSet = new Set(stableUpdaterTargets);

const argSpec = {
  "--input": { key: "input" },
  "--output|--out": { key: "output" },
  "--evidence-out": { key: "evidenceOutput" },
  "--version": { key: "version" },
  "--channel": { key: "channel" },
  "--base-url": { key: "baseUrl" },
  "--notes": { key: "notes" },
  "--pub-date": { key: "pubDate" },
  "--target": { key: "targets", list: true },
  "--placeholder-signatures": { key: "placeholderSignatures", value: true },
};

function parseOptions(argv) {
  return parseArgs(argv, argSpec, {
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
  });
}

function printHelp() {
  console.log(`Usage: pnpm release -- updater [options]

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

function normalizeBaseUrl(baseUrl, channel) {
  const stable = isStableChannel(channel);
  return normalizeChannelBaseUrl(baseUrl, channel, {
    missing: stable
      ? "Stable updater metadata requires --base-url or VOYAVPN_UPDATES_BASE_URL"
      : "Updater metadata generation requires --base-url or VOYAVPN_UPDATES_BASE_URL",
    label: stable ? "Stable updater base URL" : "Updater base URL",
    allowHttp: !stable,
    rejectPlaceholderPath: true,
  });
}

function resolveBaseUrl(options) {
  const configuredBaseUrl = options.baseUrl ?? process.env.VOYAVPN_UPDATES_BASE_URL;
  if (configuredBaseUrl !== undefined && configuredBaseUrl !== null) {
    return normalizeBaseUrl(configuredBaseUrl, options.channel);
  }

  if (isStableChannel(options.channel)) {
    return normalizeBaseUrl(null, options.channel);
  }

  return normalizeBaseUrl(`https://cdn.voyavpn.test/${options.channel}/updater`, options.channel);
}

function describeStableTarget(target) {
  const details = stableTargets.find((entry) => entry.updater === target);

  return {
    target,
    os: details?.os ?? null,
    arch: details?.arch ?? null,
  };
}

function placeholderToken(target) {
  return `VOYAVPN_UPDATER_SIGNATURE_PLACEHOLDER_${target.toUpperCase().replace(/[^A-Z0-9]+/g, "_")}`;
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

async function verifyArtifactFile(manifestDir, artifact, context, requireManifestMetadata) {
  const path = safeArtifactPath(artifact, context);
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
  const actualSha256 = await sha256File(fullPath);

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
  safeArtifactPath(artifact, context);

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
  const unsupported = keys.filter((target) => !stableUpdaterTargetSet.has(target));
  if (unsupported.length > 0) {
    throw new Error(`Stable updater metadata contains unsupported target(s): ${unsupported.join(", ")}`);
  }
}

function buildTargetEvidence(evidence) {
  return evidence.map((entry) => ({
    ...describeStableTarget(entry.target),
    source: entry.source,
    ...artifactEvidence(
      [
        { name: entry.artifact, originalName: entry.sourceArtifactName, bytes: entry.bytes, sha256: entry.sha256 },
        {
          name: entry.signatureArtifact,
          originalName: entry.sourceSignatureArtifactName,
          bytes: entry.signatureBytes,
          sha256: entry.signatureSha256,
        },
      ].filter((artifact) => artifact.name),
    ),
  }));
}

async function loadManifests(inputDir) {
  const manifests = [];
  for (const manifestPath of await walkArtifactManifests(inputDir)) {
    const manifest = readJson(manifestPath);
    manifests.push({
      manifestPath,
      manifestDir: dirname(manifestPath),
      manifest,
    });
  }
  return manifests;
}

async function readSignature(manifestDir, signatureArtifact) {
  const signaturePath = resolve(manifestDir, safeArtifactPath(signatureArtifact, "signature artifact"));
  return (await readFile(signaturePath, "utf8")).trim();
}

async function main(argv = []) {
  const options = parseOptions(argv);
  if (options.help) {
    printHelp();
    return;
  }
  if (isStableChannel(options.channel) && options.placeholderSignatures) {
    throw new Error("Stable updater metadata cannot use --placeholder-signatures; use a dry-run channel for placeholders.");
  }

  const version = options.version ?? (await readPackageVersion(repoRoot));
  const baseUrl = resolveBaseUrl(options);
  const notes = options.notes ?? `VoyaVPN ${version} ${options.channel} release`;
  const pubDate = options.pubDate ?? new Date().toISOString();
  const inputDir = resolve(repoRoot, options.input);
  const outputPath = resolve(repoRoot, options.output);
  const evidencePath = resolve(repoRoot, options.evidenceOutput ?? defaultEvidencePath(outputPath));
  const updaterPublicKey = isStableChannel(options.channel) ? resolveApprovedUpdaterPublicKey() : null;

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
    if (isStableChannel(options.channel) && current.manifestDir && current.manifestDir !== loaded.manifestDir) {
      throw new Error(`Stable updater metadata has multiple manifests for target ${target}`);
    }
    current.artifacts.push(...loaded.manifest.artifacts);
    current.manifestDir = loaded.manifestDir;
    targets.set(target, current);
  }

  if (targets.size === 0) {
    throw new Error("No target manifests found. Pass --target with --placeholder-signatures for dry-run metadata.");
  }

  if (isStableChannel(options.channel)) {
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
    const payload = selectUpdaterPayload(targetArtifacts.artifacts);
    const signatureArtifact = payload ? findSignatureArtifact(payload, targetArtifacts.artifacts) : null;

    if (payload && signatureArtifact) {
      const requireManifestMetadata = isStableChannel(options.channel);
      if (isStableChannel(options.channel)) {
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
      if (isStableChannel(options.channel)) {
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

  if (isStableChannel(options.channel)) {
    assertStableTargetMatrix(
      Object.keys(platforms),
      stableUpdaterTargets,
      "Stable updater metadata is missing signed payloads for target(s)",
    );
  }

  const generatedAt = new Date().toISOString();
  const targetEvidence = buildTargetEvidence(evidence);
  const firstStableTargets = targetEvidence
    .map((target) => target.target)
    .filter((target) => stableUpdaterTargetSet.has(target));
  const evidenceDocument = {
    channel: options.channel,
    version,
    baseUrl,
    generatedAt,
    latestPath: outputPath,
    evidencePath,
    sourceInput: sourceInputEvidence(inputDir, repoRoot),
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
      signedArtifactsRequiredForStable: isStableChannel(options.channel),
      stableTargetMatrixComplete: isStableChannel(options.channel),
      updaterPublicKeyApproved: isStableChannel(options.channel),
      updaterSignaturesVerified: isStableChannel(options.channel)
        ? evidence.every((entry) => entry.source === "signed-artifact" && entry.signatureVerified === true)
        : false,
    },
    targets: targetEvidence,
    evidence,
    platforms: evidencePlatforms,
  };

  if (isStableChannel(options.channel)) {
    assertStableDocuments(latest, evidenceDocument, baseUrl);
  }

  writeJson(outputPath, latest);
  writeJson(evidencePath, evidenceDocument);

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

export { main, printHelp };
