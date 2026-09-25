import { copyFile, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { basename, join, relative, resolve } from "node:path";
import { parseArgs } from "../../lib/args.mjs";
import { readPackageVersion, repoRootFromScript } from "../../lib/common.mjs";
import { sha256File, sha256Text, walkFiles, writeJson } from "../../lib/fs.mjs";
import { isStableChannel, placeholderText } from "../validation.mjs";

const repoRoot = repoRootFromScript(import.meta.url);

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

const argSpec = {
  "--input": { key: "input" },
  "--output": { key: "output" },
  "--target": { key: "target" },
  "--channel": { key: "channel" },
  "--version": { key: "version" },
  "--product": { key: "product" },
  "--stable-updater-config": { key: "stableUpdaterConfig" },
  "--allow-empty": { key: "allowEmpty", value: true },
};

function parseOptions(argv) {
  return parseArgs(argv, argSpec, {
    input: null,
    output: "dist/release",
    target: null,
    channel: "beta",
    version: null,
    product: "VoyaVPN",
    allowEmpty: false,
    stableUpdaterConfig: null,
  });
}

function printHelp() {
  console.log(`Usage: pnpm release -- artifacts --input <bundle-dir> --target <platform> [options]

Options:
  --output <dir>     Directory for normalized artifacts and manifests. Default: dist/release
  --channel <name>   Release channel label used in artifact names. Default: beta
  --version <semver> App version. Defaults to package.json version
  --product <name>   Product name used in artifact names. Default: VoyaVPN
  --stable-updater-config <file>
                    Stable updater overlay used by the package build.
                    Default for stable: target/release-config/tauri.updater.stable.generated.json
  --allow-empty      Write empty manifests instead of failing when no bundle artifacts exist

The designated Tauri 2 updater payload (Windows NSIS -setup.exe, Linux .AppImage,
macOS .app.tar.gz with a sibling .sig) is marked \`updaterPayload: true\` in the
manifest; a stable collection fails when the bundle has none.`);
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

export function classifyArtifact(filePath, inputDir, suffix) {
  const relativePath = relative(inputDir, filePath).replaceAll("\\", "/").toLowerCase();
  const lowerSuffix = suffix.toLowerCase();

  if (lowerSuffix.endsWith(".sig")) {
    return "signature";
  }

  // Bundler directory names are matched as path segments: `nsis/...` at the
  // root of the bundle dir has no leading slash, so `includes("/nsis/")` missed
  // every real bundle tree.
  if (relativePath.endsWith(".app.tar.gz")) {
    return "updater";
  }

  switch (lowerSuffix) {
    case ".dmg":
      return "dmg";
    case ".msi":
      return "msi";
    case ".exe":
      return /(^|\/)nsis\//.test(relativePath) ? "nsis" : "setup";
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

/**
 * The bundle path pattern identifying the current updater payload for each OS.
 *
 * With Tauri 2's `createUpdaterArtifacts: true` the updater payload is the
 * installer itself, signed in place next to a sibling `.sig`; only macOS emits a
 * separate `.app.tar.gz`. Several artifacts of one target can therefore be
 * signed (Windows NSIS *and* MSI, Linux AppImage *and* .deb/.rpm), so the
 * designated installer type is chosen deliberately here instead of by suffix:
 * NSIS on Windows (the only installer type the overlay's
 * `plugins.updater.windows.installMode` configures) and AppImage on Linux.
 */
const updaterPayloadPatterns = {
  darwin: /\.app\.tar\.gz$/i,
  linux: /\.appimage$/i,
  windows: /-setup\.exe$/i,
};

export function updaterPayloadPlatform(target) {
  const value = String(target ?? "").trim().toLowerCase();
  if (value.startsWith("darwin") || value.startsWith("macos")) {
    return "darwin";
  }
  if (value.startsWith("windows") || value.startsWith("win")) {
    return "windows";
  }
  if (value.startsWith("linux")) {
    return "linux";
  }
  return null;
}

/**
 * Returns the bundle-relative path of the updater payload for `target`, or null
 * when the bundle has no signed payload (an unsigned or dry-run build).
 */
export function selectUpdaterPayloadPath(relativePaths, target) {
  const platform = updaterPayloadPlatform(target);
  if (!platform) {
    return null;
  }

  const signed = new Set(
    relativePaths
      .filter((path) => path.toLowerCase().endsWith(".sig"))
      .map((path) => path.slice(0, -4).toLowerCase()),
  );

  const pattern = updaterPayloadPatterns[platform];
  return relativePaths.find((path) => pattern.test(path) && signed.has(path.toLowerCase())) ?? null;
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

async function stableUpdaterConfigEvidence(options, outputDir) {
  const configuredPath =
    options.stableUpdaterConfig ??
    process.env.VOYAVPN_STABLE_UPDATER_CONFIG_PATH ??
    (isStableChannel(options.channel) ? "target/release-config/tauri.updater.stable.generated.json" : null);
  if (!configuredPath) {
    return null;
  }

  const sourcePath = resolve(repoRoot, configuredPath);
  let sourceText;
  try {
    sourceText = await readFile(sourcePath, "utf8");
  } catch (error) {
    if (error && error.code === "ENOENT" && !isStableChannel(options.channel)) {
      return null;
    }
    throw new Error(`Stable updater config overlay is required for stable artifacts: ${sourcePath}`, { cause: error });
  }

  const overlay = JSON.parse(sourceText);
  const updater = overlay.plugins?.updater;
  const endpoints = Array.isArray(updater?.endpoints) ? updater.endpoints.map((endpoint) => String(endpoint)) : [];
  const pubkey = typeof updater?.pubkey === "string" ? updater.pubkey.trim() : "";
  const createUpdaterArtifacts = overlay.bundle?.createUpdaterArtifacts === true;

  if (isStableChannel(options.channel)) {
    if (!createUpdaterArtifacts) {
      throw new Error("Stable updater config overlay must enable bundle.createUpdaterArtifacts.");
    }
    if (placeholderText(pubkey) || pubkey.length < 32) {
      throw new Error("Stable updater config overlay must contain the approved non-placeholder updater public key.");
    }
    if (endpoints.length === 0 || endpoints.some((endpoint) => placeholderText(endpoint))) {
      throw new Error("Stable updater config overlay must contain non-placeholder updater endpoints.");
    }
  }

  const copiedName = "stable-updater-config.json";
  const copiedPath = join(outputDir, copiedName);
  if (resolve(copiedPath) !== sourcePath) {
    await writeFile(copiedPath, sourceText);
  }

  return {
    path: copiedName,
    sourcePath: relative(repoRoot, sourcePath).replaceAll("\\", "/"),
    sha256: sha256Text(sourceText),
    pubkeySha256: sha256Text(pubkey),
    endpoints,
    createUpdaterArtifacts,
  };
}

async function main(argv = []) {
  const options = parseOptions(argv);
  if (options.help) {
    printHelp();
    return;
  }
  if (!options.input) {
    throw new Error("--input is required");
  }
  if (!options.target) {
    throw new Error("--target is required");
  }

  const version = options.version ?? (await readPackageVersion(repoRoot));
  const inputDir = resolve(repoRoot, options.input);
  const outputDir = resolve(repoRoot, options.output);
  const productSlug = slugify(options.product);
  const channelSlug = slugify(options.channel);
  const targetSlug = slugify(options.target);

  await mkdir(outputDir, { recursive: true });
  const stableUpdaterConfig = await stableUpdaterConfigEvidence(options, outputDir);

  const sourceFiles = (await walkFiles(inputDir))
    .map((file) => ({ file, suffix: artifactSuffix(basename(file)) }))
    .filter((entry) => entry.suffix !== null)
    .sort((left, right) => relative(inputDir, left.file).localeCompare(relative(inputDir, right.file)));

  if (sourceFiles.length === 0 && !options.allowEmpty) {
    throw new Error(`No release artifacts found under ${inputDir}`);
  }

  const names = new Map();
  const artifacts = [];
  const relativePaths = sourceFiles.map(({ file }) => relative(inputDir, file).replaceAll("\\", "/"));
  const updaterPayloadPath = selectUpdaterPayloadPath(relativePaths, options.target);

  if (!updaterPayloadPath && sourceFiles.length > 0 && isStableChannel(options.channel)) {
    throw new Error(
      `No signed updater payload found for ${options.target} under ${relative(repoRoot, inputDir)}. ` +
        "Stable bundles must contain the designated installer (Windows NSIS -setup.exe, Linux .AppImage, " +
        "macOS .app.tar.gz) next to its .sig.",
    );
  }

  // Detached signatures are named after the artifact they sign, so a published
  // `<payload>.sig` stays discoverable next to its payload even when a target
  // ships several signed installers.
  const normalizedNames = new Map();
  for (const { file, suffix } of sourceFiles) {
    const originalRelativePath = relative(inputDir, file).replaceAll("\\", "/");
    if (suffix.toLowerCase().endsWith(".sig")) {
      continue;
    }
    const kind = classifyArtifact(file, inputDir, suffix);
    normalizedNames.set(
      originalRelativePath,
      nextUniqueName(names, `${productSlug}-${version}-${channelSlug}-${targetSlug}-${kind}${suffix}`, suffix),
    );
  }
  for (const { file, suffix } of sourceFiles) {
    const originalRelativePath = relative(inputDir, file).replaceAll("\\", "/");
    if (!suffix.toLowerCase().endsWith(".sig")) {
      continue;
    }
    const signedName = normalizedNames.get(originalRelativePath.slice(0, -4));
    normalizedNames.set(
      originalRelativePath,
      signedName
        ? `${signedName}.sig`
        : nextUniqueName(names, `${productSlug}-${version}-${channelSlug}-${targetSlug}-signature${suffix}`, suffix),
    );
  }

  for (const { file, suffix } of sourceFiles) {
    const kind = classifyArtifact(file, inputDir, suffix);
    const originalRelativePath = relative(inputDir, file).replaceAll("\\", "/");
    const name = normalizedNames.get(originalRelativePath);
    const destination = join(outputDir, name);

    await copyFile(file, destination);

    const fileStat = await stat(destination);
    const hash = await sha256File(destination);
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
      originalRelativePath,
      // The updater payload and its detached signature are marked explicitly:
      // with Tauri 2 in-place updater artifacts, "which file does the updater
      // serve" cannot be derived from the suffix alone.
      ...(updaterPayloadPath && originalRelativePath === updaterPayloadPath ? { updaterPayload: true } : {}),
      ...(updaterPayloadPath && originalRelativePath === `${updaterPayloadPath}.sig`
        ? { updaterSignature: true }
        : {}),
    });
  }

  const manifest = {
    productName: options.product,
    version,
    channel: options.channel,
    target: options.target,
    generatedAt: new Date().toISOString(),
    sourceBundleDir: relative(repoRoot, inputDir).replaceAll("\\", "/"),
    ...(updaterPayloadPath ? { updaterPayloadSource: updaterPayloadPath } : {}),
    ...(stableUpdaterConfig ? { stableUpdaterConfig } : {}),
    artifacts,
  };

  const checksumLines = artifacts.map((artifact) => `${artifact.sha256}  ${artifact.name}`);
  await writeFile(join(outputDir, "SHA256SUMS"), `${checksumLines.join("\n")}${checksumLines.length ? "\n" : ""}`);
  writeJson(join(outputDir, "artifact-manifest.json"), manifest);

  console.log(`Collected ${artifacts.length} artifact(s) for ${options.target} in ${relative(repoRoot, outputDir)}`);
}

export { main, printHelp };
