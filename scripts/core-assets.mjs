import { mkdir, readFile, writeFile } from "node:fs/promises";
import { basename, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const stableChannel = "stable";

const coreTypes = new Map([
  ["xray", "Xray"],
  ["Xray", "Xray"],
  ["mihomo", "mihomo"],
  ["sing-box", "sing_box"],
  ["sing_box", "sing_box"],
]);

const stableCoreTypes = ["Xray", "mihomo", "sing_box"];
const stableOs = ["windows", "macos", "linux"];
const stableArchs = ["x64", "arm64"];
const archiveFormats = new Set(["zip", "tar.gz", "gz"]);

function parseArgs(argv) {
  const options = {
    fixture: null,
    output: "dist/release/core-assets.json",
    evidenceOutput: null,
    baseUrl: null,
    channel: stableChannel,
    product: "VoyaVPN",
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
      case "--fixture":
        options.fixture = next();
        break;
      case "--output":
      case "--out":
        options.output = next();
        break;
      case "--evidence-out":
        options.evidenceOutput = next();
        break;
      case "--base-url":
        options.baseUrl = next();
        break;
      case "--channel":
        options.channel = next();
        break;
      case "--product":
        options.product = next();
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
  console.log(`Usage: node scripts/core-assets.mjs --fixture <core-assets.json> --out <manifest.json> [options]

Generates a stable CDN core asset manifest for Xray, mihomo, and sing-box.
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

Stable validation rejects unsupported OS/architecture pairs, unknown core types,
missing Xray/mihomo/sing_box entries for the Windows/macOS/Linux x64+arm64 matrix,
example or GitHub CDN base URLs, and GitHub download URLs outside upstreamUrl.`);
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
    host.includes("githubusercontent.com") ||
    host === "github.io" ||
    host.endsWith(".github.io")
  );
}

function isGithubHost(hostname) {
  const host = hostname.toLowerCase();
  return host === "github.com" || host.endsWith(".github.com") || host.includes("githubusercontent.com");
}

function normalizeBaseUrl(baseUrl, channel) {
  const value = (baseUrl ?? "").trim();
  if (!value) {
    throw new Error(
      isStable(channel)
        ? "Stable core asset manifest generation requires --base-url or VOYAVPN_CDN_BASE_URL"
        : "Core asset manifest generation requires --base-url or VOYAVPN_CDN_BASE_URL",
    );
  }

  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    throw new Error(`Invalid CDN base URL: ${value}`);
  }

  if (parsed.protocol !== "https:" && parsed.protocol !== "http:") {
    throw new Error(`CDN base URL must use http or https: ${value}`);
  }

  parsed.hash = "";
  parsed.search = "";

  if (isStable(channel) && isForbiddenStableHost(parsed.hostname)) {
    throw new Error(`Stable CDN base URL must not use example or GitHub hosts: ${value}`);
  }

  return parsed.toString().replace(/\/+$/g, "");
}

function joinUrl(baseUrl, artifactPath) {
  const segments = String(artifactPath)
    .split("/")
    .filter((segment) => segment.length > 0)
    .map((segment) => encodeURIComponent(segment));

  return [baseUrl, ...segments].join("/");
}

function defaultEvidencePath(outputPath) {
  const name = basename(outputPath);
  const dot = name.lastIndexOf(".");
  const evidenceName = dot === -1 ? `${name}.evidence.json` : `${name.slice(0, dot)}.evidence.json`;
  return resolve(dirname(outputPath), evidenceName);
}

function requiredString(value, field, context) {
  if (!value || typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${context} is missing ${field}`);
  }
  const trimmed = value.trim();
  if (/placeholder/i.test(trimmed)) {
    throw new Error(`${context} contains placeholder ${field}`);
  }
  return trimmed;
}

function requiredSha256(value, context) {
  const hash = requiredString(value, "sha256", context).toLowerCase();
  if (!/^[a-f0-9]{64}$/.test(hash)) {
    throw new Error(`${context} has invalid sha256: ${value}`);
  }
  if (/^([a-f0-9])\1{63}$/.test(hash)) {
    throw new Error(`${context} has placeholder-like sha256: ${value}`);
  }
  return hash;
}

function requiredBytes(value, context) {
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${context} has invalid bytes: ${value}`);
  }
  return value;
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
  if (isForbiddenStableHost(parsed.hostname)) {
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

  if (isStable(channel) && !asset.url.startsWith(`${baseUrl}/`)) {
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
    if (isGithubHost(url.hostname) || isForbiddenStableHost(url.hostname)) {
      throw new Error(`Stable core asset URL must be an own-CDN URL: ${asset.url}`);
    }
    if (!asset.url.startsWith(`${baseUrl}/`)) {
      throw new Error(`Stable core asset URL is not derived from CDN base URL: ${asset.url}`);
    }
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
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

  if (assets.length === 0) {
    throw new Error("Core asset fixture did not contain any assets");
  }

  assertConsistentCoreFields(assets);
  if (isStable(options.channel)) {
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

  if (isStable(options.channel)) {
    assertStableManifest(manifest, baseUrl);
  }

  const evidence = {
    productName: options.product,
    channel: options.channel,
    baseUrl,
    generatedAt,
    coreManifestPath: outputPath,
    sourceFixture: fixturePath,
    assetCount: assets.length,
    validations: {
      urlsDerivedFromBaseUrl: true,
      githubUrlsOnlyInUpstreamReferences: true,
      firstStableMatrixComplete: isStable(options.channel),
      requiredAssetFieldsPresent: true,
    },
    assets: assets.map((asset) => ({
      coreType: asset.coreType,
      version: asset.version,
      license: asset.license,
      os: asset.os,
      arch: asset.arch,
      archiveFormat: asset.archiveFormat,
      name: asset.name,
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

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
