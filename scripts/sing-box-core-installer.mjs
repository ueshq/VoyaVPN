import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
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
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const DEFAULT_SING_BOX_VERSION = "v1.13.14";
export const SING_BOX_REPO = "SagerNet/sing-box";
export const SING_BOX_CORE_DIR = "sing_box";

const SING_BOX_OS = {
  darwin: "darwin",
  linux: "linux",
  win32: "windows",
};

const SING_BOX_ARCH = {
  arm64: "arm64",
  x64: "amd64",
};

export function repoRootFromScript(importMetaUrl = import.meta.url) {
  return resolve(dirname(fileURLToPath(importMetaUrl)), "..");
}

export function truthy(value) {
  return /^(1|true|yes|on)$/i.test(String(value ?? "").trim());
}

export function singBoxVersionLabel(version = DEFAULT_SING_BOX_VERSION) {
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

export function isSingBoxPayloadFile(name) {
  return /^sing-box(\.exe)?$/i.test(name) || /^licen[cs]e(\..*)?$/i.test(name);
}

export function seedRoot(repoRoot) {
  return join(repoRoot, "src-tauri", "resources", "core-seeds");
}

export function singBoxSeedDir(repoRoot) {
  return join(seedRoot(repoRoot), SING_BOX_CORE_DIR);
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

export function singBoxAppBinDir(appConfigDir) {
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

export function ensureExecutablePermission(path, platform = process.platform) {
  if (platform === "win32" || !existsSync(path)) {
    return;
  }

  const mode = statSync(path).mode;
  chmodSync(path, mode | 0o755);
}

export function probeSingBoxExecutable(path, { platform = process.platform, spawn = spawnSync } = {}) {
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

export function copyDirectoryContents(sourceDir, targetDir) {
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

export function copySingBoxSeedToAppData({
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

export function copySingBoxAppDataToSeed({
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

export async function download(url, destFile, { fetchImpl = fetch } = {}) {
  const response = await fetchImpl(url, {
    headers: { "User-Agent": "voyavpn-sing-box-core-installer" },
    redirect: "follow",
  });
  if (!response.ok) {
    throw new Error(`download failed ${response.status} ${response.statusText}: ${url}`);
  }

  const buffer = Buffer.from(await response.arrayBuffer());
  writeFileSync(destFile, buffer);

  return buffer;
}

export function extractArchive(archiveFile, destDir, { platform = process.platform, spawn = spawnSync } = {}) {
  mkdirSync(destDir, { recursive: true });
  const command =
    platform === "win32"
      ? {
          args: ["-NoProfile", "-Command", `Expand-Archive -Path "${archiveFile}" -DestinationPath "${destDir}" -Force`],
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

export function stageExtractedSingBoxPayload(extractDir, destinationSeedDir, { platform = process.platform } = {}) {
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
  fetchImpl = fetch,
  logger = console,
  platform = process.platform,
  repoRoot,
  spawn = spawnSync,
  version = process.env.SING_BOX_VERSION ?? DEFAULT_SING_BOX_VERSION,
} = {}) {
  const assetName = singBoxAssetName({ arch, platform, version });
  if (!assetName) {
    throw new Error(`no sing-box asset is configured for ${platform}:${arch}`);
  }

  const url = `https://github.com/${SING_BOX_REPO}/releases/download/${version}/${assetName}`;
  logger.log(`- sing-box: ${SING_BOX_REPO} ${version} (${assetName})`);

  const tempDir = mkdtempSync(join(tmpdir(), "voyavpn-sing-box-core-"));
  try {
    const archiveFile = join(tempDir, assetName);
    const buffer = await download(url, archiveFile, { fetchImpl });
    const sha256 = createHash("sha256").update(buffer).digest("hex");

    const extractDir = join(tempDir, "extract");
    extractArchive(archiveFile, extractDir, { platform, spawn });

    const destinationSeedDir = singBoxSeedDir(repoRoot);
    const kept = stageExtractedSingBoxPayload(extractDir, destinationSeedDir, { platform });
    const manifest = {
      assetName,
      bytes: buffer.length,
      fetchedAt: new Date().toISOString(),
      kept,
      sha256,
      upstreamUrl: url,
      version,
    };
    writeFileSync(join(destinationSeedDir, "sing-box.seed.json"), `${JSON.stringify(manifest, null, 2)}\n`);
    logger.log(`  ✓ staged ${kept.join(", ")} -> ${relative(repoRoot, destinationSeedDir)}/`);
    logger.log(`  ✓ SHA256 ${sha256}`);

    return { assetName, kept, seedDir: destinationSeedDir, sha256, url, version };
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
  version = env.SING_BOX_VERSION ?? DEFAULT_SING_BOX_VERSION,
} = {}) {
  const skip = shouldSkipSingBoxInstall({ env, postinstall });
  if (skip.skip) {
    logger.log(`Skipping sing-box install: ${skip.reason}`);
    return { reason: skip.reason, status: "skipped" };
  }

  const resolvedAppConfigDir = appConfigDir ?? defaultAppConfigDir({ env, platform });
  const appExecutable = singBoxAppExecutable(resolvedAppConfigDir, platform);
  const effectiveForceFetch = forceFetch ?? truthy(env.VOYAVPN_FORCE_SING_BOX_FETCH);

  if (!forceInstall && !effectiveForceFetch && probeExecutable(appExecutable)) {
    if (!hasExpectedSingBoxExecutable(singBoxSeedDir(repoRoot), platform)) {
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

  if (effectiveForceFetch || !hasExpectedSingBoxExecutable(singBoxSeedDir(repoRoot), platform)) {
    await stageSeed({ arch, fetchImpl, logger, platform, repoRoot, spawn, version });
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

export async function ensureSingBoxSeedForBuild({
  arch = process.arch,
  env = process.env,
  logger = console,
  platform = process.platform,
  repoRoot,
  spawn = spawnSync,
  stageSeed = fetchAndStageSingBoxSeed,
  version = env.SING_BOX_VERSION ?? DEFAULT_SING_BOX_VERSION,
} = {}) {
  if (hasExpectedSingBoxExecutable(singBoxSeedDir(repoRoot), platform)) {
    return { seedDir: singBoxSeedDir(repoRoot), status: "already-staged" };
  }

  const result = await stageSeed({ arch, logger, platform, repoRoot, spawn, version });
  return { ...result, status: "staged" };
}

export function parseInstallArgs(argv) {
  return {
    forceFetch: argv.includes("--force-fetch"),
    forceInstall: argv.includes("--force"),
  };
}

export function isCliEntrypoint(importMetaUrl) {
  return process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href === importMetaUrl : false;
}
