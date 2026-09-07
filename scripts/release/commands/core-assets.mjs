import { mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, resolve } from "node:path";
import { parseArgs } from "../../lib/args.mjs";
import { repoRootFromScript } from "../../lib/common.mjs";
import { stableCoreTypes, stableTargets } from "../matrix.mjs";
import {
  defaultEvidencePath,
  isForbiddenStableHost,
  isStableChannel,
  joinUrl,
  normalizeReleaseUrl,
  requiredBytes,
  requiredNonPlaceholderString,
  requiredSha256 as sharedRequiredSha256,
  sourceInputEvidence,
  uniqueSorted,
} from "../validation.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const stableChannel = "stable";

const coreTypes = new Map();

const stableOs = [...new Set(stableTargets.map((target) => target.os))];
const stableArchs = [...new Set(stableTargets.map((target) => target.arch))];
const stableCoreTargetMatrix = stableTargets.map(({ os, arch, releaseTarget }) => ({
  target: `${os}-${arch}`,
  releaseTarget,
  os,
  arch,
}));
const archiveFormats = new Set(["zip", "tar.gz", "gz"]);

const argSpec = {
  "--fixture": { key: "fixture" },
  "--output|--out": { key: "output" },
  "--evidence-out": { key: "evidenceOutput" },
  "--base-url": { key: "baseUrl" },
  "--channel": { key: "channel" },
  "--product": { key: "product" },
};

function parseOptions(argv) {
  return parseArgs(argv, argSpec, {
    fixture: null,
    output: "dist/release/core-assets.json",
    evidenceOutput: null,
    baseUrl: null,
    channel: stableChannel,
    product: "VoyaVPN",
  });
}

function printHelp() {
  console.log(`Usage: pnpm release -- core-assets --fixture <core-assets.json> --out <manifest.json> [options]

Generates a stable CDN core asset manifest. The current stable release does not
publish downloadable core updates; sing-box is bundled as an application seed.
Production download URLs are derived only from --base-url or VOYAVPN_CDN_BASE_URL.
GitHub URLs are allowed only in upstreamUrl source-reference fields.

Options:
  --fixture <file>         Core asset fixture JSON input
  --out <file>             Core manifest JSON output path. Default: dist/release/core-assets.json
  --evidence-out <file>    Evidence JSON output path. Default: sibling *.evidence.json
  --base-url <url>         CDN base URL. Stable requires this or VOYAVPN_CDN_BASE_URL
  --channel <name>         Release channel. Default: stable
  --product <name>         Product name recorded in the manifest. Default: VoyaVPN

Required stable asset fields:
  coreType, version, license, os, arch, archiveFormat, executableCandidates,
  path or name, sha256, bytes, and upstreamUrl.

Stable validation rejects unsupported core asset entries, example or GitHub CDN
base URLs, and GitHub download URLs outside upstreamUrl.`);
}

function isGithubHost(hostname) {
  const host = hostname.toLowerCase();
  return host === "github.com" || host.endsWith(".github.com") || host.includes("githubusercontent.com");
}

function normalizeBaseUrl(baseUrl, channel) {
  const value = (baseUrl ?? "").trim();
  if (!value) {
    throw new Error(
      isStableChannel(channel)
        ? "Stable core asset manifest generation requires --base-url or VOYAVPN_CDN_BASE_URL"
        : "Core asset manifest generation requires --base-url or VOYAVPN_CDN_BASE_URL",
    );
  }

  // The dry-run lane generates "stable" metadata against the placeholder `.test`
  // CDN, so local/test hosts stay allowed here.
  return normalizeReleaseUrl(value, {
    allowHttp: true,
    allowTestHosts: true,
    checkHost: isStableChannel(channel),
    label: isStableChannel(channel) ? "Stable CDN base URL" : "CDN base URL",
  });
}

function requiredString(value, field, context) {
  return requiredNonPlaceholderString(value, field, context);
}

function requiredSha256(value, context) {
  return sharedRequiredSha256(value, context, { rejectRepeatedDigits: true });
}

function normalizeCoreType(value, context) {
  const coreType = requiredString(value, "coreType", context);
  const canonical = coreTypes.get(coreType) ?? coreTypes.get(coreType.toLowerCase());
  if (!canonical || !stableCoreTypes.includes(canonical)) {
    throw new Error(`${context} has unknown coreType: ${coreType}`);
  }
  return canonical;
}

function normalizeOs(value, context) {
  const os = requiredString(value, "os", context).toLowerCase();
  const normalized =
    {
      win32: "windows",
      win: "windows",
      windows: "windows",
      darwin: "macos",
      osx: "macos",
      macos: "macos",
      linux: "linux",
    }[os] ?? os;

  if (!stableOs.includes(normalized)) {
    throw new Error(`${context} has unsupported first-stable os: ${value}`);
  }
  return normalized;
}

function normalizeArch(value, context) {
  const arch = requiredString(value, "arch", context).toLowerCase();
  const normalized =
    {
      amd64: "x64",
      "x86_64": "x64",
      x64: "x64",
      aarch64: "arm64",
      arm64: "arm64",
    }[arch] ?? arch;

  if (!stableArchs.includes(normalized)) {
    throw new Error(`${context} has unsupported first-stable arch: ${value}`);
  }
  return normalized;
}

function normalizeArchiveFormat(value, context) {
  const format = requiredString(value, "archiveFormat", context).toLowerCase();
  if (!archiveFormats.has(format)) {
    throw new Error(`${context} has unsupported archiveFormat: ${value}`);
  }
  return format;
}

function validateArtifactPath(artifactPath, context) {
  if (!artifactPath || typeof artifactPath !== "string") {
    throw new Error(`${context} is missing path or name`);
  }
  if (
    artifactPath.startsWith("/") ||
    artifactPath.includes("\\") ||
    artifactPath.includes("?") ||
    artifactPath.includes("#") ||
    artifactPath.split("/").some((segment) => segment === "..")
  ) {
    throw new Error(`${context} has an unsafe artifact path: ${artifactPath}`);
  }
  return artifactPath;
}

function requiredExecutableCandidates(value, context) {
  if (!Array.isArray(value) || value.length === 0) {
    throw new Error(`${context} is missing executableCandidates[]`);
  }

  return value.map((candidate, index) => {
    const executable = requiredString(candidate, `executableCandidates[${index}]`, context);
    if (executable.includes("/") || executable.includes("\\") || executable === "." || executable === "..") {
      throw new Error(`${context} has unsafe executable candidate: ${executable}`);
    }
    return executable;
  });
}

function requiredUrl(value, field, context) {
  const url = requiredString(value, field, context);
  let parsed;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error(`${context} has invalid ${field}: ${url}`);
  }

  if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
    throw new Error(`${context} ${field} must use http or https: ${url}`);
  }

  return parsed;
}

function rejectProductionDownloadUrl(value, context) {
  if (!value) {
    return;
  }

  const parsed = requiredUrl(value, "download URL", context);
  if (isGithubHost(parsed.hostname)) {
    throw new Error(`${context} uses a GitHub production download URL; move it to upstreamUrl instead`);
  }
  if (isForbiddenStableHost(parsed.hostname, { allowTestHosts: true })) {
    throw new Error(`${context} uses a forbidden production download URL: ${value}`);
  }
}

function normalizeEntry(entry, index, baseUrl, channel) {
  const context = `assets[${index}]`;
  const coreType = normalizeCoreType(entry.coreType, context);
  const os = normalizeOs(entry.os, context);
  const arch = normalizeArch(entry.arch, context);
  const archiveFormat = normalizeArchiveFormat(entry.archiveFormat ?? entry.archive?.format, context);
  const path = validateArtifactPath(entry.path ?? entry.archive?.path ?? entry.name, context);
  const upstream = requiredUrl(entry.upstreamUrl ?? entry.upstream?.url, "upstreamUrl", context);

  if (entry.url || entry.downloadUrl || entry.cdnUrl) {
    rejectProductionDownloadUrl(entry.url ?? entry.downloadUrl ?? entry.cdnUrl, context);
  }

  const asset = {
    coreType,
    version: requiredString(entry.version, "version", context),
    license: requiredString(entry.license, "license", context),
    os,
    arch,
    archiveFormat,
    executableCandidates: requiredExecutableCandidates(entry.executableCandidates, context),
    url: joinUrl(baseUrl, path),
    sha256: requiredSha256(entry.sha256, context),
    bytes: requiredBytes(entry.bytes, context),
    upstreamUrl: upstream.toString(),
    name: requiredString(entry.name ?? basename(path), "name", context),
    path,
  };

  if (isStableChannel(channel) && !asset.url.startsWith(`${baseUrl}/`)) {
    throw new Error(`${context} URL is not derived from CDN base URL: ${asset.url}`);
  }

  return asset;
}

function assetSort(left, right) {
  const coreComparison = stableCoreTypes.indexOf(left.coreType) - stableCoreTypes.indexOf(right.coreType);
  if (coreComparison !== 0) {
    return coreComparison;
  }

  const osComparison = stableOs.indexOf(left.os) - stableOs.indexOf(right.os);
  if (osComparison !== 0) {
    return osComparison;
  }

  const archComparison = stableArchs.indexOf(left.arch) - stableArchs.indexOf(right.arch);
  if (archComparison !== 0) {
    return archComparison;
  }

  return left.name.localeCompare(right.name);
}

function assertStableCompleteness(assets) {
  const seen = new Map();
  for (const asset of assets) {
    const key = `${asset.coreType}/${asset.os}/${asset.arch}`;
    if (seen.has(key)) {
      throw new Error(`Duplicate stable core asset entry for ${key}`);
    }
    seen.set(key, asset);
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
    throw new Error(`Stable core manifest is missing required entries: ${missing.join(", ")}`);
  }
}

function assertConsistentCoreFields(assets) {
  const byCore = new Map();
  for (const asset of assets) {
    const current = byCore.get(asset.coreType) ?? { version: asset.version, license: asset.license };
    if (current.version !== asset.version) {
      throw new Error(`${asset.coreType} mixes versions: ${current.version}, ${asset.version}`);
    }
    if (current.license !== asset.license) {
      throw new Error(`${asset.coreType} mixes licenses: ${current.license}, ${asset.license}`);
    }
    byCore.set(asset.coreType, current);
  }
}

function assertStableManifest(manifest, baseUrl) {
  for (const asset of manifest.assets) {
    const url = new URL(asset.url);
    if (isGithubHost(url.hostname) || isForbiddenStableHost(url.hostname, { allowTestHosts: true })) {
      throw new Error(`Stable core asset URL must be an own-CDN URL: ${asset.url}`);
    }
    if (!asset.url.startsWith(`${baseUrl}/`)) {
      throw new Error(`Stable core asset URL is not derived from CDN base URL: ${asset.url}`);
    }
  }
}

function coreVersions(assets) {
  return Object.fromEntries(
    stableCoreTypes
      .map((coreType) => {
        const asset = assets.find((entry) => entry.coreType === coreType);
        return asset ? [coreType, asset.version] : null;
      })
      .filter(Boolean),
  );
}

function stableTargetForAsset(asset) {
  return stableCoreTargetMatrix.find((target) => target.os === asset.os && target.arch === asset.arch) ?? {
    target: `${asset.os}-${asset.arch}`,
    releaseTarget: null,
    os: asset.os,
    arch: asset.arch,
  };
}

function stableTargetRank(targetName) {
  const index = stableCoreTargetMatrix.findIndex((target) => target.target === targetName);
  return index === -1 ? stableCoreTargetMatrix.length : index;
}

function buildTargetEvidence(assets) {
  const byTarget = new Map();

  for (const asset of assets) {
    const target = stableTargetForAsset(asset);
    const current = byTarget.get(target.target) ?? {
      ...target,
      assets: [],
    };
    current.assets.push(asset);
    byTarget.set(target.target, current);
  }

  return [...byTarget.values()]
    .sort((left, right) => {
      const rankComparison = stableTargetRank(left.target) - stableTargetRank(right.target);
      return rankComparison !== 0 ? rankComparison : left.target.localeCompare(right.target);
    })
    .map((target) => ({
      target: target.target,
      releaseTarget: target.releaseTarget,
      os: target.os,
      arch: target.arch,
      assetCount: target.assets.length,
      coreTypes: stableCoreTypes.filter((coreType) => target.assets.some((asset) => asset.coreType === coreType)),
      sourceArtifactNames: uniqueSorted(target.assets.map((asset) => asset.name)),
      checksums: target.assets.map((asset) => ({
        coreType: asset.coreType,
        version: asset.version,
        name: asset.name,
        sourceArtifactName: asset.name,
        path: asset.path,
        bytes: asset.bytes,
        sha256: asset.sha256,
      })),
    }));
}

function buildCoreTargetEvidence(assets) {
  return stableCoreTypes.map((coreType) => {
    const coreAssets = assets.filter((asset) => asset.coreType === coreType);
    const targets = buildTargetEvidence(coreAssets);
    return {
      coreType,
      version: coreAssets[0]?.version ?? null,
      license: coreAssets[0]?.license ?? null,
      targetCount: targets.length,
      sourceArtifactNames: uniqueSorted(coreAssets.map((asset) => asset.name)),
      targets: targets.map((target) => ({
        target: target.target,
        releaseTarget: target.releaseTarget,
        os: target.os,
        arch: target.arch,
        sourceArtifactNames: target.sourceArtifactNames,
        checksums: target.checksums,
      })),
    };
  });
}

async function main(argv = []) {
  const options = parseOptions(argv);
  if (options.help) {
    printHelp();
    return;
  }
  if (!options.fixture) {
    throw new Error("--fixture is required");
  }

  const fixturePath = resolve(repoRoot, options.fixture);
  const outputPath = resolve(repoRoot, options.output);
  const evidencePath = resolve(repoRoot, options.evidenceOutput ?? defaultEvidencePath(outputPath));
  const baseUrl = normalizeBaseUrl(options.baseUrl ?? process.env.VOYAVPN_CDN_BASE_URL, options.channel);
  const fixture = JSON.parse(await readFile(fixturePath, "utf8"));

  if (!Array.isArray(fixture.assets)) {
    throw new Error(`${fixturePath} is missing assets[]`);
  }

  const assets = fixture.assets.map((entry, index) => normalizeEntry(entry, index, baseUrl, options.channel)).sort(assetSort);

  assertConsistentCoreFields(assets);
  if (isStableChannel(options.channel)) {
    assertStableCompleteness(assets);
  }

  const generatedAt = requiredString(fixture.generatedAt ?? "1970-01-01T00:00:00.000Z", "generatedAt", "fixture");
  const manifest = {
    productName: options.product,
    manifestVersion: 1,
    channel: options.channel,
    baseUrl,
    generatedAt,
    assets,
  };

  if (isStableChannel(options.channel)) {
    assertStableManifest(manifest, baseUrl);
  }

  const targetEvidence = buildTargetEvidence(assets);
  const firstStableTargets = targetEvidence
    .map((target) => target.target)
    .filter((target) => stableCoreTargetMatrix.some((stableTarget) => stableTarget.target === target));
  const evidence = {
    productName: options.product,
    manifestVersion: manifest.manifestVersion,
    channel: options.channel,
    versions: coreVersions(assets),
    baseUrl,
    generatedAt,
    coreManifestPath: outputPath,
    evidencePath,
    sourceInput: sourceInputEvidence(fixturePath, repoRoot, "operator-supplied-core-assets"),
    sourceFixture: fixturePath,
    assetCount: assets.length,
    targetCount: targetEvidence.length,
    firstStableTargetCount: firstStableTargets.length,
    firstStableTargets,
    coreTypeCount: uniqueSorted(assets.map((asset) => asset.coreType)).length,
    checksumCount: assets.length,
    sourceArtifactNames: uniqueSorted(assets.map((asset) => asset.name)),
    validations: {
      urlsDerivedFromBaseUrl: true,
      githubUrlsOnlyInUpstreamReferences: true,
      firstStableMatrixComplete: isStableChannel(options.channel),
      requiredAssetFieldsPresent: true,
    },
    targets: targetEvidence,
    coreTargets: buildCoreTargetEvidence(assets),
    assets: assets.map((asset) => ({
      coreType: asset.coreType,
      version: asset.version,
      license: asset.license,
      os: asset.os,
      arch: asset.arch,
      archiveFormat: asset.archiveFormat,
      name: asset.name,
      sourceArtifactName: asset.name,
      sourceArtifactPath: asset.path,
      bytes: asset.bytes,
      sha256: asset.sha256,
      url: asset.url,
      upstreamUrl: asset.upstreamUrl,
    })),
  };

  await mkdir(dirname(outputPath), { recursive: true });
  await mkdir(dirname(evidencePath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(manifest, null, 2)}\n`);
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);

  console.log(`Wrote core asset manifest to ${outputPath}`);
  console.log(`Wrote core asset evidence to ${evidencePath}`);
}

export { main, printHelp };
