import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function parseArgs(argv) {
  const options = {
    input: "dist/release",
    output: "dist/release/latest.json",
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
  --version <semver>           App version. Defaults to package.json version
  --channel <name>             Release channel. Default: beta
  --base-url <url>             Public update asset base URL. Default: https://updates.voyavpn.example/<channel>
  --notes <text>               Release notes string
  --pub-date <iso>             Publication timestamp. Default: current time
  --target <platform[,..]>     Platform key to include when no manifest exists
  --placeholder-signatures     Emit dry-run placeholder updater URLs and signatures`);
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
  for (const entry of entries) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      manifests.push(...(await walkManifests(path)));
    } else if (entry.isFile() && entry.name === "artifact-manifest.json") {
      manifests.push(path);
    }
  }
  return manifests;
}

function joinUrl(baseUrl, ...parts) {
  const cleanBase = baseUrl.replace(/\/+$/g, "");
  const cleanParts = parts.map((part) => encodeURIComponent(part).replaceAll("%2F", "/"));
  return [cleanBase, ...cleanParts].join("/");
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
  const signaturePath = resolve(manifestDir, signatureArtifact.path);
  return (await readFile(signaturePath, "utf8")).trim();
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const version = options.version ?? (await readPackageVersion());
  const baseUrl = options.baseUrl ?? `https://updates.voyavpn.example/${options.channel}`;
  const notes = options.notes ?? `VoyaVPN ${version} ${options.channel} release`;
  const pubDate = options.pubDate ?? new Date().toISOString();
  const inputDir = resolve(repoRoot, options.input);
  const outputPath = resolve(repoRoot, options.output);

  const loadedManifests = await loadManifests(inputDir);
  const targets = new Map();

  for (const target of options.targets) {
    targets.set(target, { target, artifacts: [], manifestDir: null });
  }

  for (const loaded of loadedManifests) {
    const target = loaded.manifest.target;
    const current = targets.get(target) ?? { target, artifacts: [], manifestDir: loaded.manifestDir };
    current.artifacts.push(...loaded.manifest.artifacts);
    current.manifestDir = loaded.manifestDir;
    targets.set(target, current);
  }

  if (targets.size === 0) {
    throw new Error("No target manifests found. Pass --target with --placeholder-signatures for dry-run metadata.");
  }

  const platforms = {};
  const evidence = [];

  for (const target of [...targets.keys()].sort()) {
    const targetArtifacts = targets.get(target);
    const payload = targetArtifacts.artifacts.find(isUpdaterPayload);
    const signatureArtifact = payload ? findSignatureArtifact(payload, targetArtifacts.artifacts) : null;

    if (payload && signatureArtifact) {
      platforms[target] = {
        signature: await readSignature(targetArtifacts.manifestDir, signatureArtifact),
        url: joinUrl(baseUrl, payload.name),
      };
      evidence.push({ target, source: "signed-artifact", artifact: payload.name });
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
    evidence.push({ target, source: "placeholder", artifact: placeholderName });
  }

  const latest = {
    version,
    notes,
    pub_date: pubDate,
    platforms,
  };

  await mkdir(dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(latest, null, 2)}\n`);
  await writeFile(
    join(dirname(outputPath), "latest.evidence.json"),
    `${JSON.stringify({ channel: options.channel, baseUrl, generatedAt: new Date().toISOString(), evidence }, null, 2)}\n`,
  );

  console.log(`Wrote updater metadata to ${relative(repoRoot, outputPath)}`);
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
