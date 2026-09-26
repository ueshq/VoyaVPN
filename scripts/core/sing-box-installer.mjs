import { spawnSync } from "node:child_process";
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { parseArgs } from "../lib/args.mjs";
import { truthy } from "../lib/common.mjs";
import { readJson, sha256FileSync, sha256Text, writeJson } from "../lib/fs.mjs";
import { requestedMacAppStoreBuild } from "../tauri/mac-app-store-config.mjs";
import { download } from "./download.mjs";

export const DEFAULT_SING_BOX_VERSION = "v1.13.14";
const SING_BOX_REPO = "SagerNet/sing-box";
const SING_BOX_CORE_DIR = "sing_box";
export const SING_BOX_SEED_MANIFEST = "sing-box.seed.json";

const SING_BOX_VERSION_PATTERN = /^v\d+\.\d+\.\d+(-[\w.]+)?$/;
export const ALLOW_UNPINNED_SING_BOX_ENV = "VOYAVPN_ALLOW_UNPINNED_SING_BOX";
export const ALLOW_SEED_BACKFILL_ENV = "VOYAVPN_ALLOW_SING_BOX_SEED_BACKFILL";

/**
 * Where the bundled seed comes from: `upstream` (the pinned release archive,
 * the default) or `source` (built from the pinned sing-box commit by
 * `scripts/core/sing-box-source-seed.mjs`). The Mac App Store build always
 * needs `source`: the upstream macOS binary is built with
 * `with_naive_outbound`, which links Chromium's Cronet, and Cronet imports the
 * non-public `__kCFBundleNumericVersionKey` and links private `/usr/lib`
 * libraries (App Review Guideline 2.5.1). See
 * docs/release/sing-box-seed-pinning.md.
 */
export const SING_BOX_SEED_ORIGIN_ENV = "VOYAVPN_SING_BOX_SEED_ORIGIN";
const SING_BOX_SEED_ORIGINS = new Set(["upstream", "source"]);

export const SING_BOX_SOURCE_REPOSITORY = "https://github.com/SagerNet/sing-box.git";

/**
 * The commit each tag must resolve to for a source build. A tag can be moved,
 * so this is the source build's counterpart of SING_BOX_ARCHIVE_SHA256 and is
 * bumped in the same commit as DEFAULT_SING_BOX_VERSION.
 */
export const SING_BOX_SOURCE_COMMITS = {
  "v1.13.14": "25a600db24f7680ad9806ce5427bd0ab8afe1114",
};

/**
 * Build tags of the source seed: upstream's own default for a `make build`
 * (`release/DEFAULT_BUILD_TAGS_OTHERS` at the pinned tag), which already
 * leaves out `with_naive_outbound`. Written out here so a version bump that
 * changes upstream's list fails the build instead of changing the binary.
 */
export const SING_BOX_SOURCE_BUILD_TAGS = Object.freeze([
  "with_gvisor",
  "with_quic",
  "with_dhcp",
  "with_wireguard",
  "with_utls",
  "with_acme",
  "with_clash_api",
  "with_tailscale",
  "with_ccm",
  "with_ocm",
  "badlinkname",
  "tfogo_checklinkname0",
]);

/** Tags that must never be in a source seed; see SING_BOX_SEED_ORIGIN_ENV. */
export const SING_BOX_SOURCE_EXCLUDED_TAGS = Object.freeze(["with_naive_outbound"]);

/**
 * SHA-256 of the upstream *release archive* the installer downloads (the same
 * bytes `download()` hashes), keyed by version and by the `<os>-<cpu>` pair that
 * appears in the asset name.
 *
 * Source of truth: the GitHub release asset digests
 * (`GET /repos/SagerNet/sing-box/releases/tags/<version>` -> `assets[].digest`),
 * cross-checked for darwin-arm64 against the archive that produced the seed
 * manifest committed alongside this table. Bump this table in the same commit as
 * DEFAULT_SING_BOX_VERSION; the procedure is documented in
 * docs/release/sing-box-seed-pinning.md.
 *
 * A {version, platform, arch} combination that is absent here is refused unless
 * VOYAVPN_ALLOW_UNPINNED_SING_BOX=1 is set for a local experiment.
 */
const SING_BOX_ARCHIVE_SHA256 = {
  "v1.13.14": {
    "darwin-amd64": "5245d645e847f90bb708da74bc020ae078c28489690756419685c04f56b4e3bb",
    "darwin-arm64": "73e8967b0fc08e17bce4263ca56ebc394822401a16497a1c4e02316c888202ab",
    "linux-amd64": "f48703461a15476951ac4967cdad339d986f4b8096b4eb3ff0829a500502d697",
    "linux-arm64": "4742df6a4314e8ecc41736849fca6d73b8f9e91b6e8b06ee794ff17ba180579e",
    "windows-amd64": "f580782c6dd10f7691c66cea1d7c421813c5fbf7e305d1ee7ce0c3a40d196341",
    "windows-arm64": "b22b597063ccb0e2e4fff53f677fe896e882ec5560d74d8db4fca5a0fed0a7b6",
  },
};

const SING_BOX_OS = {
  darwin: "darwin",
  linux: "linux",
  win32: "windows",
};

const SING_BOX_ARCH = {
  arm64: "arm64",
  x64: "amd64",
};

function singBoxVersionLabel(version = DEFAULT_SING_BOX_VERSION) {
  return String(version).replace(/^v/i, "");
}

export function singBoxAssetName({
  arch = process.arch,
  platform = process.platform,
  version = DEFAULT_SING_BOX_VERSION,
} = {}) {
  const os = SING_BOX_OS[platform];
  const cpu = SING_BOX_ARCH[arch];
  if (!os || !cpu) {
    return null;
  }

  const extension = platform === "win32" ? "zip" : "tar.gz";
  return `sing-box-${singBoxVersionLabel(version)}-${os}-${cpu}.${extension}`;
}

export function singBoxExecutableName(platform = process.platform) {
  return platform === "win32" ? "sing-box.exe" : "sing-box";
}

export function singBoxPinKey({ arch = process.arch, platform = process.platform } = {}) {
  const os = SING_BOX_OS[platform];
  const cpu = SING_BOX_ARCH[arch];
  return os && cpu ? `${os}-${cpu}` : null;
}

export function assertSingBoxVersion(version) {
  const value = String(version ?? "").trim();
  if (!SING_BOX_VERSION_PATTERN.test(value)) {
    throw new Error(
      `SING_BOX_VERSION must look like v1.2.3 or v1.2.3-beta.1 (got ${JSON.stringify(String(version ?? ""))}).`,
    );
  }
  return value;
}

export function expectedSingBoxArchiveSha256({
  arch = process.arch,
  platform = process.platform,
  version = DEFAULT_SING_BOX_VERSION,
} = {}) {
  const key = singBoxPinKey({ arch, platform });
  if (!key) {
    return null;
  }
  return SING_BOX_ARCHIVE_SHA256[String(version)]?.[key] ?? null;
}

function unpinnedSingBoxMessage(version, platform, arch) {
  return (
    `sing-box ${version} has no pinned SHA-256 for ${platform}:${arch}. ` +
    "Add the upstream release archive digest to SING_BOX_ARCHIVE_SHA256 in " +
    "scripts/core/sing-box-installer.mjs (see docs/release/sing-box-seed-pinning.md), " +
    `or set ${ALLOW_UNPINNED_SING_BOX_ENV}=1 to stage an unverified core for a local experiment.`
  );
}

/**
 * Resolves how a {version, platform, arch} triple is allowed to be staged:
 * verified against the checked-in table, explicitly unpinned via the escape
 * hatch, or refused.
 */
export function singBoxPinStatus({
  arch = process.arch,
  env = process.env,
  platform = process.platform,
  version = DEFAULT_SING_BOX_VERSION,
} = {}) {
  const expected = expectedSingBoxArchiveSha256({ arch, platform, version });
  if (expected) {
    return { expected, pinned: true, unpinnedAllowed: false };
  }

  return {
    expected: null,
    pinned: false,
    reason: unpinnedSingBoxMessage(version, platform, arch),
    unpinnedAllowed: truthy(env[ALLOW_UNPINNED_SING_BOX_ENV]),
  };
}

/**
 * Which seed this build must bundle. The Mac App Store build always gets the
 * source seed, and asking it for the upstream one is refused rather than
 * silently shipping a binary App Review has already rejected.
 */
export function requestedSingBoxSeedOrigin(env = process.env) {
  const raw = String(env[SING_BOX_SEED_ORIGIN_ENV] ?? "").trim().toLowerCase();
  if (raw && !SING_BOX_SEED_ORIGINS.has(raw)) {
    throw new Error(`${SING_BOX_SEED_ORIGIN_ENV} must be "upstream" or "source" (got ${JSON.stringify(raw)}).`);
  }
  if (requestedMacAppStoreBuild(env)) {
    if (raw === "upstream") {
      throw new Error(
        `The Mac App Store build must bundle the source-built sing-box seed; unset ${SING_BOX_SEED_ORIGIN_ENV}=upstream.`,
      );
    }
    return "source";
  }
  return raw || "upstream";
}

/** The source build's pin for `version`, with the same escape hatch as the archive pin. */
export function singBoxSourcePinStatus({ env = process.env, version = DEFAULT_SING_BOX_VERSION } = {}) {
  const expected = SING_BOX_SOURCE_COMMITS[String(version)] ?? null;
  if (expected) {
    return { expected, pinned: true, unpinnedAllowed: false };
  }
  return {
    expected: null,
    pinned: false,
    reason:
      `sing-box ${version} has no pinned source commit. Add it to SING_BOX_SOURCE_COMMITS in ` +
      "scripts/core/sing-box-installer.mjs (see docs/release/sing-box-seed-pinning.md), " +
      `or set ${ALLOW_UNPINNED_SING_BOX_ENV}=1 to build an unverified core for a local experiment.`,
    unpinnedAllowed: truthy(env[ALLOW_UNPINNED_SING_BOX_ENV]),
  };
}

export function sameSingBoxBuildTags(actual, expected = SING_BOX_SOURCE_BUILD_TAGS) {
  if (!Array.isArray(actual)) {
    return false;
  }
  const left = [...new Set(actual.map(String))].sort();
  const right = [...new Set(expected)].sort();
  return left.length === right.length && left.every((tag, index) => tag === right[index]);
}

function assertSingBoxPinAvailable(status) {
  if (!status.pinned && !status.unpinnedAllowed) {
    throw new Error(status.reason);
  }
  return status;
}

/**
 * Compares a freshly downloaded archive against the pinned digest. Callers must
 * run this before extracting or executing anything from the archive.
 */
export function assertPinnedSingBoxArchive({
  actualSha256,
  arch = process.arch,
  assetName,
  env = process.env,
  platform = process.platform,
  version = DEFAULT_SING_BOX_VERSION,
}) {
  const status = assertSingBoxPinAvailable(singBoxPinStatus({ arch, env, platform, version }));
  if (status.pinned && actualSha256 !== status.expected) {
    throw new Error(
      `sing-box archive ${assetName} failed SHA-256 pinning: expected ${status.expected}, got ${actualSha256}`,
    );
  }

  return { pinned: status.pinned, sha256: actualSha256 };
}

export function readSingBoxSeedManifest(seedDir) {
  const manifestPath = join(seedDir, SING_BOX_SEED_MANIFEST);
  if (!existsSync(manifestPath)) {
    return null;
  }

  try {
    const manifest = readJson(manifestPath);
    return manifest && typeof manifest === "object" ? manifest : null;
  } catch {
    return null;
  }
}

/**
 * Decides whether the checked-out seed may be bundled as-is. Name-only checks
 * let a stale version, a foreign architecture, or a locally replaced binary ship
 * inside a package, so the seed is trusted only when its manifest matches the
 * expected asset and the pinned digest, and the staged executable still hashes
 * to what the manifest recorded.
 */
export function verifyStagedSingBoxSeed({
  arch = process.arch,
  env = process.env,
  origin,
  platform = process.platform,
  repoRoot,
  version = DEFAULT_SING_BOX_VERSION,
} = {}) {
  const requestedOrigin = origin ?? requestedSingBoxSeedOrigin(env);
  const seedDir = singBoxSeedDir(repoRoot);
  const executableName = singBoxExecutableName(platform);
  if (!hasExpectedSingBoxExecutable(seedDir, platform)) {
    return { code: "missing", ok: false, reason: `no ${executableName} is staged in ${seedDir}`, staged: false };
  }

  const assetName = singBoxAssetName({ arch, platform, version });
  if (!assetName) {
    return {
      code: "unsupported-target",
      ok: false,
      reason: `no sing-box asset is configured for ${platform}:${arch}`,
      staged: true,
    };
  }

  const manifest = readSingBoxSeedManifest(seedDir);
  if (!manifest) {
    return { code: "manifest-missing", ok: false, reason: `${SING_BOX_SEED_MANIFEST} is missing or unreadable`, staged: true };
  }
  // A seed staged for the other origin is stale for this build, never
  // acceptable: that is what keeps a source seed out of a Developer ID
  // package and the upstream binary out of the store package.
  const manifestOrigin = manifest.origin ?? "upstream";
  if (manifestOrigin !== requestedOrigin) {
    return {
      code: "origin-mismatch",
      ok: false,
      reason: `staged seed is the ${manifestOrigin} build, this build needs the ${requestedOrigin} one`,
      staged: true,
    };
  }
  if (requestedOrigin === "source") {
    return verifyStagedSourceSingBoxSeed({ arch, env, executableName, manifest, platform, seedDir, version });
  }
  if (manifest.version !== version) {
    return {
      code: "version-mismatch",
      ok: false,
      reason: `staged seed is ${manifest.version ?? "(unknown)"}, expected ${version}`,
      staged: true,
    };
  }
  if (manifest.assetName !== assetName) {
    return {
      code: "asset-mismatch",
      ok: false,
      reason: `staged seed is ${manifest.assetName ?? "(unknown)"}, expected ${assetName}`,
      staged: true,
    };
  }

  const status = singBoxPinStatus({ arch, env, platform, version });
  if (!/^[a-f0-9]{64}$/i.test(String(manifest.sha256 ?? "")) ||
      !/^[a-f0-9]{64}$/i.test(String(manifest.executableSha256 ?? ""))) {
    return { code: "manifest-invalid", ok: false, reason: `${SING_BOX_SEED_MANIFEST} must contain archive and executable SHA-256 digests`, staged: true };
  }
  if (status.pinned && manifest.sha256 !== status.expected) {
    return {
      code: "archive-digest-mismatch",
      ok: false,
      reason: `staged seed archive digest ${manifest.sha256 ?? "(none)"} does not match the pinned ${status.expected}`,
      staged: true,
    };
  }
  if (!status.pinned && !status.unpinnedAllowed) {
    return { code: "unpinned", ok: false, reason: status.reason, staged: true };
  }

  const digestProblem = executableDigestProblem(seedDir, executableName, manifest);
  if (digestProblem) {
    return digestProblem;
  }

  return { code: "verified", manifest, ok: true, origin: "upstream", pinned: status.pinned, reason: null, staged: true };
}

function executableDigestProblem(seedDir, executableName, manifest) {
  const actual = sha256FileSync(join(seedDir, executableName));
  if (actual.toLowerCase() === String(manifest.executableSha256).toLowerCase()) {
    return null;
  }
  return {
    code: "executable-digest-mismatch",
    ok: false,
    reason: `staged ${executableName} digest ${actual} does not match ${SING_BOX_SEED_MANIFEST}`,
    staged: true,
  };
}

/**
 * The source seed's counterpart of the archive checks: the right version and
 * target, the pinned commit, exactly the pinned tags (so never
 * `with_naive_outbound`), and an executable that still hashes to what the
 * build recorded.
 */
function verifyStagedSourceSingBoxSeed({ arch, env, executableName, manifest, platform, seedDir, version }) {
  const failure = (code, reason) => ({ code, ok: false, reason, staged: true });
  if (manifest.version !== version) {
    return failure("version-mismatch", `staged seed is ${manifest.version ?? "(unknown)"}, expected ${version}`);
  }
  const target = singBoxPinKey({ arch, platform });
  if (manifest.target !== target) {
    return failure("target-mismatch", `staged seed was built for ${manifest.target ?? "(unknown)"}, expected ${target}`);
  }
  if (!/^[a-f0-9]{40}$/i.test(String(manifest.commit ?? "")) ||
      !/^[a-f0-9]{64}$/i.test(String(manifest.executableSha256 ?? ""))) {
    return failure("manifest-invalid", `${SING_BOX_SEED_MANIFEST} must contain the source commit and executable SHA-256`);
  }
  const status = singBoxSourcePinStatus({ env, version });
  if (status.pinned && manifest.commit.toLowerCase() !== status.expected) {
    return failure(
      "source-commit-mismatch",
      `staged seed was built from ${manifest.commit}, not the pinned ${status.expected}`,
    );
  }
  if (!status.pinned && !status.unpinnedAllowed) {
    return failure("unpinned", status.reason);
  }
  const excluded = (Array.isArray(manifest.tags) ? manifest.tags : []).filter((tag) =>
    SING_BOX_SOURCE_EXCLUDED_TAGS.includes(tag));
  if (excluded.length || !sameSingBoxBuildTags(manifest.tags)) {
    return failure(
      "source-tags-mismatch",
      `staged seed was built with tags ${JSON.stringify(manifest.tags ?? null)}, expected ${SING_BOX_SOURCE_BUILD_TAGS.join(",")}`,
    );
  }
  const digestProblem = executableDigestProblem(seedDir, executableName, manifest);
  if (digestProblem) {
    return digestProblem;
  }
  return { code: "verified", manifest, ok: true, origin: "source", pinned: status.pinned, reason: null, staged: true };
}

function isSingBoxPayloadFile(name) {
  return /^sing-box(\.exe)?$/i.test(name) || /^licen[cs]e(\..*)?$/i.test(name);
}

/** Everything a package bundles from `resources/core-seeds`: the core and the rule sets. */
export function coreSeedsDir(repoRoot) {
  return join(repoRoot, "apps", "desktop", "src-tauri", "resources", "core-seeds");
}

export function singBoxSeedDir(repoRoot) {
  return join(coreSeedsDir(repoRoot), SING_BOX_CORE_DIR);
}

export function defaultAppConfigDir({
  env = process.env,
  home = homedir(),
  platform = process.platform,
} = {}) {
  if (env.VOYAVPN_APP_CONFIG_DIR?.trim()) {
    return resolve(env.VOYAVPN_APP_CONFIG_DIR.trim());
  }

  if (platform === "darwin") {
    return join(home, "Library", "Application Support", "app.voyavpn.desktop");
  }

  if (platform === "win32") {
    const root = env.APPDATA || env.USERPROFILE;
    if (!root) {
      throw new Error("APPDATA or USERPROFILE is required to locate the VoyaVPN app config directory on Windows.");
    }
    return join(root, "app.voyavpn.desktop");
  }

  const configRoot = env.XDG_CONFIG_HOME?.trim() || join(home, ".config");
  return join(configRoot, "app.voyavpn.desktop");
}

function singBoxAppBinDir(appConfigDir) {
  return join(appConfigDir, "bin", SING_BOX_CORE_DIR);
}

export function singBoxAppExecutable(appConfigDir, platform = process.platform) {
  return join(singBoxAppBinDir(appConfigDir), singBoxExecutableName(platform));
}

export function shouldSkipSingBoxInstall({ env = process.env, postinstall = false } = {}) {
  if (truthy(env.VOYAVPN_SKIP_SING_BOX_POSTINSTALL)) {
    return { reason: "VOYAVPN_SKIP_SING_BOX_POSTINSTALL=1", skip: true };
  }

  if (postinstall && truthy(env.CI) && !truthy(env.VOYAVPN_FETCH_SING_BOX_ON_INSTALL)) {
    return { reason: "CI postinstall without VOYAVPN_FETCH_SING_BOX_ON_INSTALL=1", skip: true };
  }

  return { reason: null, skip: false };
}

export function hasExpectedSingBoxExecutable(dir, platform = process.platform) {
  const executable = join(dir, singBoxExecutableName(platform));
  return existsSync(executable) && statSync(executable).isFile();
}

function ensureExecutablePermission(path, platform = process.platform) {
  if (platform === "win32" || !existsSync(path)) {
    return;
  }

  const mode = statSync(path).mode;
  chmodSync(path, mode | 0o755);
}

function probeSingBoxExecutable(path, { platform = process.platform, spawn = spawnSync } = {}) {
  if (!existsSync(path) || !statSync(path).isFile()) {
    return false;
  }

  ensureExecutablePermission(path, platform);
  const result = spawn(path, ["version"], {
    encoding: "utf8",
    timeout: 10_000,
  });
  const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;

  return result.status === 0 && /sing-box/i.test(output);
}

function copyDirectoryContents(sourceDir, targetDir) {
  mkdirSync(targetDir, { recursive: true });
  for (const entry of readdirSync(sourceDir, { withFileTypes: true })) {
    const sourcePath = join(sourceDir, entry.name);
    const targetPath = join(targetDir, entry.name);
    if (entry.isDirectory()) {
      copyDirectoryContents(sourcePath, targetPath);
    } else if (entry.isFile()) {
      cpSync(sourcePath, targetPath);
    }
  }
}

function copySingBoxSeedToAppData({
  appConfigDir,
  logger = console,
  platform = process.platform,
  repoRoot,
} = {}) {
  const sourceDir = singBoxSeedDir(repoRoot);
  const targetDir = singBoxAppBinDir(appConfigDir);
  if (!hasExpectedSingBoxExecutable(sourceDir, platform)) {
    return { copied: false, sourceDir, targetDir };
  }

  copyDirectoryContents(sourceDir, targetDir);
  ensureExecutablePermission(join(targetDir, singBoxExecutableName(platform)), platform);
  logger.log(`  ✓ copied sing-box seed -> ${targetDir}`);

  return { copied: true, sourceDir, targetDir };
}

function copySingBoxAppDataToSeed({
  appConfigDir,
  logger = console,
  platform = process.platform,
  repoRoot,
} = {}) {
  const sourceDir = singBoxAppBinDir(appConfigDir);
  const targetDir = singBoxSeedDir(repoRoot);
  if (!hasExpectedSingBoxExecutable(sourceDir, platform)) {
    return { copied: false, sourceDir, targetDir };
  }

  copyDirectoryContents(sourceDir, targetDir);
  ensureExecutablePermission(join(targetDir, singBoxExecutableName(platform)), platform);
  logger.log(`  ✓ copied sing-box app-data binary -> ${targetDir}`);

  return { copied: true, sourceDir, targetDir };
}

function extractArchive(archiveFile, destDir, { platform = process.platform, spawn = spawnSync } = {}) {
  mkdirSync(destDir, { recursive: true });
  // -LiteralPath with single-quoted, escaped operands: PowerShell treats a
  // double-quoted -Path as a wildcard pattern and expands `$(...)`.
  const powershellLiteral = (value) => `'${String(value).replaceAll("'", "''")}'`;
  const command =
    platform === "win32"
      ? {
          args: [
            "-NoProfile",
            "-Command",
            `Expand-Archive -LiteralPath ${powershellLiteral(archiveFile)} ` +
              `-DestinationPath ${powershellLiteral(destDir)} -Force`,
          ],
          file: "powershell",
        }
      : { args: ["-xzf", archiveFile, "-C", destDir], file: "tar" };

  const result = spawn(command.file, command.args, { stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`extraction failed (${command.file} exited ${result.status ?? "null"})`);
  }
}

function stagePayloadRecursive(sourceDir, destinationSeedDir, kept, platform) {
  for (const entry of readdirSync(sourceDir, { withFileTypes: true })) {
    const sourcePath = join(sourceDir, entry.name);
    if (entry.isDirectory()) {
      stagePayloadRecursive(sourcePath, destinationSeedDir, kept, platform);
      continue;
    }
    if (!entry.isFile() || !isSingBoxPayloadFile(entry.name)) {
      continue;
    }

    const targetName = /^sing-box(\.exe)?$/i.test(entry.name)
      ? singBoxExecutableName(platform)
      : entry.name;
    cpSync(sourcePath, join(destinationSeedDir, targetName));
    kept.push(targetName);
  }
}

function stageExtractedSingBoxPayload(extractDir, destinationSeedDir, { platform = process.platform } = {}) {
  rmSync(destinationSeedDir, { force: true, recursive: true });
  mkdirSync(destinationSeedDir, { recursive: true });

  const kept = [];
  stagePayloadRecursive(extractDir, destinationSeedDir, kept, platform);

  const executable = join(destinationSeedDir, singBoxExecutableName(platform));
  if (!kept.length || !existsSync(executable)) {
    throw new Error(`no ${singBoxExecutableName(platform)} executable found in extracted sing-box archive`);
  }

  ensureExecutablePermission(executable, platform);
  return kept;
}

export async function fetchAndStageSingBoxSeed({
  arch = process.arch,
  env = process.env,
  fetchImpl = fetch,
  logger = console,
  platform = process.platform,
  repoRoot,
  spawn = spawnSync,
  version = process.env.SING_BOX_VERSION ?? DEFAULT_SING_BOX_VERSION,
} = {}) {
  const resolvedVersion = assertSingBoxVersion(version);
  const assetName = singBoxAssetName({ arch, platform, version: resolvedVersion });
  if (!assetName) {
    throw new Error(`no sing-box asset is configured for ${platform}:${arch}`);
  }

  // Refuse an unpinned target before touching the network so an unsupported
  // {version, platform, arch} never downloads or executes anything.
  const pin = assertSingBoxPinAvailable(singBoxPinStatus({ arch, env, platform, version: resolvedVersion }));

  const url = `https://github.com/${SING_BOX_REPO}/releases/download/${resolvedVersion}/${assetName}`;
  logger.log(`- sing-box: ${SING_BOX_REPO} ${resolvedVersion} (${assetName})`);
  if (!pin.pinned) {
    logger.warn?.(`  ! ${ALLOW_UNPINNED_SING_BOX_ENV}=1: staging ${assetName} without an expected SHA-256`);
  }

  const tempDir = mkdtempSync(join(tmpdir(), "voyavpn-sing-box-core-"));
  try {
    const archiveFile = join(tempDir, assetName);
    const buffer = await download(url, { "User-Agent": "voyavpn-sing-box-core-installer" }, { fetchImpl });
    writeFileSync(archiveFile, buffer);
    const sha256 = sha256Text(buffer);
    const verification = assertPinnedSingBoxArchive({
      actualSha256: sha256,
      arch,
      assetName,
      env,
      platform,
      version: resolvedVersion,
    });

    const extractDir = join(tempDir, "extract");
    extractArchive(archiveFile, extractDir, { platform, spawn });

    const destinationSeedDir = singBoxSeedDir(repoRoot);
    const kept = stageExtractedSingBoxPayload(extractDir, destinationSeedDir, { platform });
    const executableSha256 = sha256FileSync(join(destinationSeedDir, singBoxExecutableName(platform)));
    const manifest = {
      assetName,
      bytes: buffer.length,
      executableSha256,
      fetchedAt: new Date().toISOString(),
      kept,
      origin: "upstream",
      pinned: verification.pinned,
      sha256,
      upstreamUrl: url,
      version: resolvedVersion,
    };
    writeJson(join(destinationSeedDir, SING_BOX_SEED_MANIFEST), manifest);
    logger.log(`  ✓ staged ${kept.join(", ")} -> ${relative(repoRoot, destinationSeedDir)}/`);
    logger.log(`  ✓ SHA256 ${sha256}${verification.pinned ? " (matches pinned digest)" : " (unpinned)"}`);

    return {
      assetName,
      executableSha256,
      kept,
      pinned: verification.pinned,
      seedDir: destinationSeedDir,
      sha256,
      url,
      version: resolvedVersion,
    };
  } finally {
    rmSync(tempDir, { force: true, recursive: true });
  }
}

export async function installSingBoxCore({
  appConfigDir,
  arch = process.arch,
  env = process.env,
  fetchImpl = fetch,
  forceFetch,
  forceInstall = false,
  logger = console,
  platform = process.platform,
  postinstall = false,
  probeExecutable = (path) => probeSingBoxExecutable(path, { platform }),
  repoRoot,
  spawn = spawnSync,
  stageSeed = fetchAndStageSingBoxSeed,
  buildSeed,
  version = env.SING_BOX_VERSION ?? DEFAULT_SING_BOX_VERSION,
} = {}) {
  const skip = shouldSkipSingBoxInstall({ env, postinstall });
  if (skip.skip) {
    logger.log(`Skipping sing-box install: ${skip.reason}`);
    return { reason: skip.reason, status: "skipped" };
  }

  const resolvedVersion = assertSingBoxVersion(version);
  const resolvedAppConfigDir = appConfigDir ?? defaultAppConfigDir({ env, platform });
  const appExecutable = singBoxAppExecutable(resolvedAppConfigDir, platform);
  const effectiveForceFetch = forceFetch ?? truthy(env.VOYAVPN_FORCE_SING_BOX_FETCH);

  if (!forceInstall && !effectiveForceFetch && probeExecutable(appExecutable)) {
    // The app-data binary is owned by the running app and can be replaced or
    // updated behind our back, so it is never promoted into the repo seed that
    // `tauri build` bundles unless a developer opts in explicitly.
    if (truthy(env[ALLOW_SEED_BACKFILL_ENV]) && !hasExpectedSingBoxExecutable(singBoxSeedDir(repoRoot), platform)) {
      copySingBoxAppDataToSeed({
        appConfigDir: resolvedAppConfigDir,
        logger,
        platform,
        repoRoot,
      });
    }
    logger.log(`sing-box already installed: ${appExecutable}`);
    return { executable: appExecutable, status: "already-installed" };
  }

  const origin = requestedSingBoxSeedOrigin(env);
  const seedVerification = verifyStagedSingBoxSeed({ arch, env, origin, platform, repoRoot, version: resolvedVersion });
  if (effectiveForceFetch || !seedVerification.ok) {
    if (!effectiveForceFetch && seedVerification.staged) {
      logger.log(`- re-staging sing-box seed: ${seedVerification.reason}`);
    }
    const stage = await seedStager({ buildSeed, origin, stageSeed });
    await stage({ arch, env, fetchImpl, logger, platform, repoRoot, spawn, version: resolvedVersion });
  }

  const copy = copySingBoxSeedToAppData({
    appConfigDir: resolvedAppConfigDir,
    logger,
    platform,
    repoRoot,
  });

  if (!copy.copied) {
    throw new Error(
      `sing-box seed was not installed because ${singBoxSeedDir(repoRoot)} has no ${singBoxExecutableName(platform)}`,
    );
  }

  if (!probeExecutable(appExecutable)) {
    throw new Error(`installed sing-box executable did not pass version probe: ${appExecutable}`);
  }

  return { executable: appExecutable, seedDir: copy.sourceDir, status: "installed" };
}

/**
 * The function that stages a seed of `origin`. The source builder is loaded
 * on demand: it needs Go and a sing-box checkout, which an ordinary
 * `pnpm install` or Developer ID build never touches.
 */
async function seedStager({ buildSeed, origin, stageSeed }) {
  if (origin !== "source") {
    return stageSeed;
  }
  return buildSeed ?? (await import("./sing-box-source-seed.mjs")).buildAndStageSingBoxSeed;
}

export async function ensureSingBoxSeedForBuild({
  arch = process.arch,
  env = process.env,
  logger = console,
  origin,
  platform = process.platform,
  repoRoot,
  spawn = spawnSync,
  stageSeed = fetchAndStageSingBoxSeed,
  buildSeed,
  version = env.SING_BOX_VERSION ?? DEFAULT_SING_BOX_VERSION,
} = {}) {
  const resolvedVersion = assertSingBoxVersion(version);
  const resolvedOrigin = origin ?? requestedSingBoxSeedOrigin(env);
  const verification = verifyStagedSingBoxSeed({
    arch,
    env,
    origin: resolvedOrigin,
    platform,
    repoRoot,
    version: resolvedVersion,
  });
  if (verification.ok) {
    return {
      origin: resolvedOrigin,
      seedDir: singBoxSeedDir(repoRoot),
      status: "already-staged",
      verified: verification.pinned,
    };
  }

  if (verification.staged) {
    logger.log(`- re-staging sing-box seed: ${verification.reason}`);
  }

  const stage = await seedStager({ buildSeed, origin: resolvedOrigin, stageSeed });
  const result = await stage({ arch, env, logger, platform, repoRoot, spawn, version: resolvedVersion });
  return { ...result, origin: resolvedOrigin, status: "staged" };
}

export function parseInstallArgs(argv) {
  return parseArgs(
    argv,
    {
      "--force": { key: "forceInstall", value: true },
      "--force-fetch": { key: "forceFetch", value: true },
    },
    { forceFetch: false, forceInstall: false },
  );
}
