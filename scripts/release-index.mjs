import { mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const stableChannel = "stable";
const stableTargets = new Set(["windows", "macos", "linux"]);
const stableArchs = new Set(["x64", "arm64"]);

function parseArgs(argv) {
  const options = {
    input: null,
    output: "dist/release/release-index.json",
    evidenceOutput: null,
    baseUrl: null,
    channel: stableChannel,
    version: null,
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
      case "--base-url":
        options.baseUrl = next();
        break;
      case "--channel":
        options.channel = next();
        break;
      case "--version":
        options.version = next();
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
  console.log(`Usage: node scripts/release-index.mjs --input <artifact-manifest-dir> --out <release-index.json> [options]

Generates a CDN release index from artifact-manifest.json files written by scripts/release-artifacts.mjs.
All artifact URLs are derived from --base-url or VOYAVPN_CDN_BASE_URL; manifest URL fields are not trusted.

Options:
  --input <dir|file>       Directory containing artifact-manifest.json files, or one manifest file
  --out <file>             Release index JSON output path. Default: dist/release/release-index.json
  --evidence-out <file>    Evidence JSON output path. Default: sibling *.evidence.json
  --base-url <url>         CDN base URL. Stable requires this or VOYAVPN_CDN_BASE_URL
  --channel <name>         Release channel. Default: stable
  --version <semver>       Expected app version. Defaults to manifest artifact versions
  --product <name>         Product name recorded in the index. Default: VoyaVPN

Required stable artifact fields:
  channel, version, target or inferable platform, arch or inferable x64/arm64,
  kind, path or name, bytes, sha256, and originalName.

Stable validation fails when the CDN base URL is missing, empty, an example host, or a GitHub URL.`);
}

async function walkManifests(root) {
  let rootStat;
  try {
    rootStat = await stat(root);
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return [];
    }
    throw error;
  }

  if (rootStat.isFile()) {
    return basename(root) === "artifact-manifest.json" ? [root] : [];
  }

  const entries = await readdir(root, { withFileTypes: true });
  const manifests = [];

  for (const entry of entries) {
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
        ? "Stable release index generation requires --base-url or VOYAVPN_CDN_BASE_URL"
        : "Release index generation requires --base-url or VOYAVPN_CDN_BASE_URL",
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

function validateArtifactPath(artifactPath, context) {
  if (!artifactPath || typeof artifactPath !== "string") {
    throw new Error(`${context} is missing path or name`);
  }

  if (artifactPath.startsWith("/") || artifactPath.split("/").some((segment) => segment === "..")) {
    throw new Error(`${context} has an unsafe artifact path: ${artifactPath}`);
  }
}

function normalizeArch(value) {
  if (!value || typeof value !== "string") {
    return null;
  }

  const lower = value.toLowerCase();
  if (/\b(aarch64|arm64)\b/.test(lower)) {
    return "arm64";
  }
  if (/\b(x86_64|amd64|x64)\b/.test(lower)) {
    return "x64";
  }
  return null;
}

function inferArch(...values) {
  for (const value of values) {
    const arch = normalizeArch(value);
    if (arch) {
      return arch;
    }
  }
  return null;
}

function normalizeTarget(value) {
  if (!value || typeof value !== "string") {
    return null;
  }

  const lower = value.toLowerCase();
  if (/\b(windows|win32|msvc)\b/.test(lower)) {
    return "windows";
  }
  if (/\b(macos|darwin|osx|apple)\b/.test(lower)) {
    return "macos";
  }
  if (/\b(linux|appimage|deb|rpm)\b/.test(lower)) {
    return "linux";
  }
  return null;
}

function inferTarget(...values) {
  for (const value of values) {
    const target = normalizeTarget(value);
    if (target) {
      return target;
    }
  }
  return null;
}

function requiredString(value, field, context) {
  if (!value || typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${context} is missing ${field}`);
  }
  return value.trim();
}

function requiredSha256(value, context) {
  const hash = requiredString(value, "sha256", context).toLowerCase();
  if (!/^[a-f0-9]{64}$/.test(hash)) {
    throw new Error(`${context} has invalid sha256: ${value}`);
  }
  return hash;
}

function requiredBytes(value, context) {
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${context} has invalid bytes: ${value}`);
  }
  return value;
}

function stableFieldCheck(entry, context) {
  if (!stableTargets.has(entry.target)) {
    throw new Error(`${context} has unsupported stable target: ${entry.target}`);
  }
  if (!stableArchs.has(entry.arch)) {
    throw new Error(`${context} has unsupported stable arch: ${entry.arch}`);
  }
}

async function loadArtifactEntries(inputDir, options, baseUrl) {
  const manifestPaths = await walkManifests(inputDir);
  if (manifestPaths.length === 0) {
    throw new Error(`No artifact-manifest.json files found under ${inputDir}`);
  }

  const artifacts = [];
  const sourceManifests = [];

  for (const manifestPath of manifestPaths) {
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    const manifestArtifacts = Array.isArray(manifest.artifacts) ? manifest.artifacts : null;
    if (!manifestArtifacts) {
      throw new Error(`${manifestPath} is missing artifacts[]`);
    }

    sourceManifests.push(relative(repoRoot, manifestPath).replaceAll("\\", "/"));

    manifestArtifacts.forEach((artifact, index) => {
      const context = `${relative(repoRoot, manifestPath)} artifacts[${index}]`;
      const sourceTarget = artifact.target ?? manifest.target;
      const explicitTarget = artifact.platform ?? manifest.platform;
      const explicitArch = artifact.arch ?? manifest.arch;
      const artifactPath = artifact.path ?? artifact.name;
      validateArtifactPath(artifactPath, context);

      const entry = {
        channel: requiredString(artifact.channel ?? manifest.channel ?? options.channel, "channel", context),
        version: requiredString(options.version ?? artifact.version ?? manifest.version, "version", context),
        target: normalizeTarget(explicitTarget) ?? explicitTarget ?? inferTarget(sourceTarget, artifact.originalName, artifact.name),
        arch: normalizeArch(explicitArch) ?? explicitArch ?? inferArch(sourceTarget, artifact.originalName, artifact.name),
        kind: requiredString(artifact.kind, "kind", context),
        url: joinUrl(baseUrl, artifactPath),
        bytes: requiredBytes(artifact.bytes, context),
        sha256: requiredSha256(artifact.sha256, context),
        originalName: requiredString(artifact.originalName, "originalName", context),
        name: requiredString(artifact.name ?? basename(artifactPath), "name", context),
      };

      if (!entry.target) {
        throw new Error(`${context} is missing target or inferable platform`);
      }
      if (!entry.arch) {
        throw new Error(`${context} is missing arch or inferable architecture`);
      }

      if (entry.channel !== options.channel) {
        throw new Error(`${context} channel ${entry.channel} does not match requested channel ${options.channel}`);
      }

      if (sourceTarget && sourceTarget !== entry.target) {
        entry.releaseTarget = sourceTarget;
      }

      if (artifact.originalRelativePath) {
        entry.originalRelativePath = artifact.originalRelativePath;
      }

      if (isStable(options.channel)) {
        stableFieldCheck(entry, context);
      }

      artifacts.push(entry);
    });
  }

  artifacts.sort((left, right) =>
    [
      left.target.localeCompare(right.target),
      left.arch.localeCompare(right.arch),
      left.kind.localeCompare(right.kind),
      left.name.localeCompare(right.name),
    ].find((comparison) => comparison !== 0) ?? 0,
  );

  return { artifacts, sourceManifests };
}

function assertSingleValue(values, field) {
  const unique = [...new Set(values)];
  if (unique.length !== 1) {
    throw new Error(`Release index cannot mix ${field} values: ${unique.join(", ")}`);
  }
  return unique[0];
}

function defaultEvidencePath(outputPath) {
  const name = basename(outputPath);
  const dot = name.lastIndexOf(".");
  const evidenceName = dot === -1 ? `${name}.evidence.json` : `${name.slice(0, dot)}.evidence.json`;
  return join(dirname(outputPath), evidenceName);
}

function assertStableIndex(index, baseUrl) {
  const serialized = JSON.stringify(index).toLowerCase();
  if (serialized.includes("github.com") || serialized.includes("voyavpn.example") || serialized.includes("placeholder")) {
    throw new Error("Stable release index contains forbidden placeholder or GitHub content");
  }

  for (const artifact of index.artifacts) {
    if (!artifact.url.startsWith(`${baseUrl}/`)) {
      throw new Error(`Artifact URL is not derived from CDN base URL: ${artifact.url}`);
    }
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (!options.input) {
    throw new Error("--input is required");
  }

  const inputDir = resolve(repoRoot, options.input);
  const outputPath = resolve(repoRoot, options.output);
  const evidencePath = resolve(repoRoot, options.evidenceOutput ?? defaultEvidencePath(outputPath));
  const baseUrl = normalizeBaseUrl(options.baseUrl ?? process.env.VOYAVPN_CDN_BASE_URL, options.channel);
  const { artifacts, sourceManifests } = await loadArtifactEntries(inputDir, options, baseUrl);

  if (artifacts.length === 0) {
    throw new Error("No artifacts found in artifact manifests");
  }

  const version = assertSingleValue(
    artifacts.map((artifact) => artifact.version),
    "version",
  );
  const channel = assertSingleValue(
    artifacts.map((artifact) => artifact.channel),
    "channel",
  );

  const generatedAt = new Date().toISOString();
  const index = {
    productName: options.product,
    channel,
    version,
    baseUrl,
    generatedAt,
    artifacts,
  };

  if (isStable(channel)) {
    assertStableIndex(index, baseUrl);
  }

  const evidence = {
    productName: options.product,
    channel,
    version,
    baseUrl,
    generatedAt,
    releaseIndexPath: outputPath,
    sourceManifests,
    artifactCount: artifacts.length,
    validations: {
      urlsDerivedFromBaseUrl: true,
      stableRejectsExampleAndGithubBaseUrls: isStable(channel),
      requiredArtifactFieldsPresent: true,
    },
    artifacts: artifacts.map((artifact) => ({
      target: artifact.target,
      arch: artifact.arch,
      kind: artifact.kind,
      name: artifact.name,
      originalName: artifact.originalName,
      bytes: artifact.bytes,
      sha256: artifact.sha256,
      url: artifact.url,
    })),
  };

  await mkdir(dirname(outputPath), { recursive: true });
  await mkdir(dirname(evidencePath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(index, null, 2)}\n`);
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);

  console.log(`Wrote release index to ${outputPath}`);
  console.log(`Wrote release evidence to ${evidencePath}`);
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
