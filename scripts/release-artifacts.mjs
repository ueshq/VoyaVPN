import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { copyFile, mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const artifactSuffixes = [
  ".tar.gz.sig",
  ".tar.gz",
  ".AppImage.sig",
  ".AppImage",
  ".dmg.sig",
  ".dmg",
  ".msi.sig",
  ".msi",
  ".exe.sig",
  ".exe",
  ".deb.sig",
  ".deb",
  ".rpm.sig",
  ".rpm",
  ".zip.sig",
  ".zip",
];

function parseArgs(argv) {
  const options = {
    input: null,
    output: "dist/release",
    target: null,
    channel: "beta",
    version: null,
    product: "VoyaVPN",
    allowEmpty: false,
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
        options.output = next();
        break;
      case "--target":
        options.target = next();
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
      case "--allow-empty":
        options.allowEmpty = true;
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
  console.log(`Usage: node scripts/release-artifacts.mjs --input <bundle-dir> --target <platform> [options]

Options:
  --output <dir>     Directory for normalized artifacts and manifests. Default: dist/release
  --channel <name>   Release channel label used in artifact names. Default: beta
  --version <semver> App version. Defaults to package.json version
  --product <name>   Product name used in artifact names. Default: VoyaVPN
  --allow-empty      Write empty manifests instead of failing when no bundle artifacts exist`);
}

async function readPackageVersion() {
  const packageJson = JSON.parse(await readFile(resolve(repoRoot, "package.json"), "utf8"));
  return packageJson.version;
}

async function walkFiles(root) {
  let entries;
  try {
    entries = await readdir(root, { withFileTypes: true });
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return [];
    }
    throw error;
  }

  const files = [];
  for (const entry of entries) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await walkFiles(path)));
    } else if (entry.isFile()) {
      files.push(path);
    }
  }
  return files;
}

function artifactSuffix(filename) {
  const lowerName = filename.toLowerCase();
  return artifactSuffixes.find((suffix) => lowerName.endsWith(suffix.toLowerCase())) ?? null;
}

function slugify(value) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function classifyArtifact(filePath, inputDir, suffix) {
  const relativePath = relative(inputDir, filePath).replaceAll("\\", "/").toLowerCase();
  const isSignature = suffix.toLowerCase().endsWith(".sig");
  const payloadSuffix = isSignature ? suffix.slice(0, -4) : suffix;

  if (isSignature) {
    return "signature";
  }

  if (payloadSuffix === ".tar.gz" || payloadSuffix === ".zip" || relativePath.includes("/updater/")) {
    return "updater";
  }

  switch (payloadSuffix.toLowerCase()) {
    case ".dmg":
      return "dmg";
    case ".msi":
      return "msi";
    case ".exe":
      return relativePath.includes("/nsis/") ? "nsis" : "setup";
    case ".deb":
      return "deb";
    case ".rpm":
      return "rpm";
    case ".appimage":
      return "appimage";
    default:
      return "artifact";
  }
}

function nextUniqueName(state, requestedName, suffix) {
  const key = requestedName.toLowerCase();
  const count = state.get(key) ?? 0;
  state.set(key, count + 1);

  if (count === 0) {
    return requestedName;
  }

  return `${requestedName.slice(0, -suffix.length)}-${count + 1}${suffix}`;
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

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (!options.input) {
    throw new Error("--input is required");
  }
  if (!options.target) {
    throw new Error("--target is required");
  }

  const version = options.version ?? (await readPackageVersion());
  const inputDir = resolve(repoRoot, options.input);
  const outputDir = resolve(repoRoot, options.output);
  const productSlug = slugify(options.product);
  const channelSlug = slugify(options.channel);
  const targetSlug = slugify(options.target);

  await mkdir(outputDir, { recursive: true });

  const sourceFiles = (await walkFiles(inputDir))
    .map((file) => ({ file, suffix: artifactSuffix(basename(file)) }))
    .filter((entry) => entry.suffix !== null)
    .sort((left, right) => relative(inputDir, left.file).localeCompare(relative(inputDir, right.file)));

  if (sourceFiles.length === 0 && !options.allowEmpty) {
    throw new Error(`No release artifacts found under ${inputDir}`);
  }

  const names = new Map();
  const artifacts = [];

  for (const { file, suffix } of sourceFiles) {
    const kind = classifyArtifact(file, inputDir, suffix);
    const requestedName = `${productSlug}-${version}-${channelSlug}-${targetSlug}-${kind}${suffix}`;
    const name = nextUniqueName(names, requestedName, suffix);
    const destination = join(outputDir, name);

    await copyFile(file, destination);

    const fileStat = await stat(destination);
    const hash = await sha256(destination);
    artifacts.push({
      name,
      path: name,
      kind,
      target: options.target,
      channel: options.channel,
      version,
      bytes: fileStat.size,
      sha256: hash,
      originalName: basename(file),
      originalRelativePath: relative(inputDir, file).replaceAll("\\", "/"),
    });
  }

  const manifest = {
    productName: options.product,
    version,
    channel: options.channel,
    target: options.target,
    generatedAt: new Date().toISOString(),
    sourceBundleDir: relative(repoRoot, inputDir).replaceAll("\\", "/"),
    artifacts,
  };

  const checksumLines = artifacts.map((artifact) => `${artifact.sha256}  ${artifact.name}`);
  await writeFile(join(outputDir, "SHA256SUMS"), `${checksumLines.join("\n")}${checksumLines.length ? "\n" : ""}`);
  await writeFile(join(outputDir, "artifact-manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);

  console.log(`Collected ${artifacts.length} artifact(s) for ${options.target} in ${relative(repoRoot, outputDir)}`);
}

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});
