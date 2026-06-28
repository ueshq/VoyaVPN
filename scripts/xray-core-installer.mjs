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

export const DEFAULT_XRAY_VERSION = "v26.3.27";
export const XRAY_REPO = "XTLS/Xray-core";
export const XRAY_CORE_DIR = "xray";

export const XRAY_ASSETS = {
  "win32:x64": "Xray-windows-64.zip",
  "win32:arm64": "Xray-windows-arm64-v8a.zip",
  "darwin:x64": "Xray-macos-64.zip",
  "darwin:arm64": "Xray-macos-arm64-v8a.zip",
  "linux:x64": "Xray-linux-64.zip",
  "linux:arm64": "Xray-linux-arm64-v8a.zip",
};

export function repoRootFromScript(importMetaUrl = import.meta.url) {
  return resolve(dirname(fileURLToPath(importMetaUrl)), "..");
}

export function truthy(value) {
  return /^(1|true|yes|on)$/i.test(String(value ?? "").trim());
}

export function xrayAssetName({ platform = process.platform, arch = process.arch } = {}) {
  return XRAY_ASSETS[`${platform}:${arch}`] ?? null;
}

export function xrayExecutableName(platform = process.platform) {
  return platform === "win32" ? "xray.exe" : "xray";
}

export function isXrayPayloadFile(name) {
  return /^xray(\.exe)?$/i.test(name) || /\.dat$/i.test(name);
}

export function seedRoot(repoRoot) {
  return join(repoRoot, "src-tauri", "resources", "core-seeds");
}

export function xraySeedDir(repoRoot) {
  return join(seedRoot(repoRoot), XRAY_CORE_DIR);
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

export function xrayAppBinDir(appConfigDir) {
  return join(appConfigDir, "bin", XRAY_CORE_DIR);
}

export function xrayAppExecutable(appConfigDir, platform = process.platform) {
  return join(xrayAppBinDir(appConfigDir), xrayExecutableName(platform));
}

export function shouldSkipXrayInstall({ env = process.env, postinstall = false } = {}) {
  if (truthy(env.VOYAVPN_SKIP_XRAY_POSTINSTALL)) {
    return { reason: "VOYAVPN_SKIP_XRAY_POSTINSTALL=1", skip: true };
  }

  if (postinstall && truthy(env.CI) && !truthy(env.VOYAVPN_FETCH_XRAY_ON_INSTALL)) {
    return { reason: "CI postinstall without VOYAVPN_FETCH_XRAY_ON_INSTALL=1", skip: true };
  }

  return { reason: null, skip: false };
}

export function hasExpectedXrayExecutable(dir, platform = process.platform) {
  const executable = join(dir, xrayExecutableName(platform));
  return existsSync(executable) && statSync(executable).isFile();
}

export function ensureExecutablePermission(path, platform = process.platform) {
  if (platform === "win32" || !existsSync(path)) {
    return;
  }

  const mode = statSync(path).mode;
  chmodSync(path, mode | 0o755);
}

export function probeXrayExecutable(path, { platform = process.platform, spawn = spawnSync } = {}) {
  if (!existsSync(path) || !statSync(path).isFile()) {
    return false;
  }

  ensureExecutablePermission(path, platform);
  const result = spawn(path, ["-version"], {
    encoding: "utf8",
    timeout: 10_000,
  });
  const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;

  return result.status === 0 && /xray/i.test(output);
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

export function copyXraySeedToAppData({
  appConfigDir,
  logger = console,
  platform = process.platform,
  repoRoot,
} = {}) {
  const sourceDir = xraySeedDir(repoRoot);
  const targetDir = xrayAppBinDir(appConfigDir);
  if (!hasExpectedXrayExecutable(sourceDir, platform)) {
    return { copied: false, sourceDir, targetDir };
  }

  copyDirectoryContents(sourceDir, targetDir);
  ensureExecutablePermission(join(targetDir, xrayExecutableName(platform)), platform);
  logger.log(`  ✓ copied Xray seed -> ${targetDir}`);

  return { copied: true, sourceDir, targetDir };
}

export function copyXrayAppDataToSeed({
  appConfigDir,
  logger = console,
  platform = process.platform,
  repoRoot,
} = {}) {
  const sourceDir = xrayAppBinDir(appConfigDir);
  const targetDir = xraySeedDir(repoRoot);
  if (!hasExpectedXrayExecutable(sourceDir, platform)) {
    return { copied: false, sourceDir, targetDir };
  }

  copyDirectoryContents(sourceDir, targetDir);
  ensureExecutablePermission(join(targetDir, xrayExecutableName(platform)), platform);
  logger.log(`  ✓ copied Xray app-data binary -> ${targetDir}`);

  return { copied: true, sourceDir, targetDir };
}

export async function download(url, destFile, { fetchImpl = fetch } = {}) {
  const response = await fetchImpl(url, {
    headers: { "User-Agent": "voyavpn-xray-core-installer" },
    redirect: "follow",
  });
  if (!response.ok) {
    throw new Error(`download failed ${response.status} ${response.statusText}: ${url}`);
  }

  const buffer = Buffer.from(await response.arrayBuffer());
  writeFileSync(destFile, buffer);

  return buffer;
}

export async function verifyChecksum(buffer, dgstUrl, { fetchImpl = fetch, logger = console } = {}) {
  try {
    const response = await fetchImpl(dgstUrl, {
      headers: { "User-Agent": "voyavpn-xray-core-installer" },
    });
    if (!response.ok) {
      logger.warn(`  ! checksum file unavailable (${response.status}); skipping verification`);
      return { skipped: true };
    }

    const text = await response.text();
    const match = text.match(/sha2?-?256[^0-9a-f]*([0-9a-f]{64})/i) ?? text.match(/\b([0-9a-f]{64})\b/i);
    if (!match) {
      logger.warn("  ! could not parse SHA256 from checksum file; skipping verification");
      return { skipped: true };
    }

    const expected = match[1].toLowerCase();
    const actual = createHash("sha256").update(buffer).digest("hex");
    if (expected !== actual) {
      throw new Error(`checksum mismatch: expected ${expected}, got ${actual}`);
    }

    logger.log("  ✓ SHA256 verified");
    return { actual, expected, skipped: false };
  } catch (error) {
    if (error.message.startsWith("checksum mismatch")) {
      throw error;
    }
    logger.warn(`  ! checksum verification skipped: ${error.message}`);
    return { skipped: true };
  }
}

export function extractZip(zipFile, destDir, { platform = process.platform, spawn = spawnSync } = {}) {
  mkdirSync(destDir, { recursive: true });
  const command =
    platform === "win32"
      ? {
          args: ["-NoProfile", "-Command", `Expand-Archive -Path "${zipFile}" -DestinationPath "${destDir}" -Force`],
          file: "powershell",
        }
      : { args: ["-o", zipFile, "-d", destDir], file: "unzip" };

  const result = spawn(command.file, command.args, { stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`extraction failed (${command.file} exited ${result.status ?? "null"})`);
  }
}

export function stageExtractedXrayPayload(extractDir, destinationSeedDir, { platform = process.platform } = {}) {
  rmSync(destinationSeedDir, { force: true, recursive: true });
  mkdirSync(destinationSeedDir, { recursive: true });

  const kept = [];
  for (const entry of readdirSync(extractDir, { recursive: true, withFileTypes: true })) {
    if (!entry.isFile() || !isXrayPayloadFile(entry.name)) {
      continue;
    }

    const parentPath = entry.parentPath ?? entry.path;
    const sourcePath = join(parentPath, entry.name);
    const targetPath = join(destinationSeedDir, entry.name);
    cpSync(sourcePath, targetPath);
    kept.push(entry.name);
  }

  const executable = join(destinationSeedDir, xrayExecutableName(platform));
  if (!kept.length || !existsSync(executable)) {
    throw new Error(`no ${xrayExecutableName(platform)} executable found in extracted Xray archive`);
  }

  ensureExecutablePermission(executable, platform);
  return kept;
}

export async function fetchAndStageXraySeed({
  arch = process.arch,
  fetchImpl = fetch,
  logger = console,
  platform = process.platform,
  repoRoot,
  spawn = spawnSync,
  version = process.env.XRAY_VERSION ?? DEFAULT_XRAY_VERSION,
} = {}) {
  const assetName = xrayAssetName({ arch, platform });
  if (!assetName) {
    throw new Error(`no Xray asset is configured for ${platform}:${arch}`);
  }

  const url = `https://github.com/${XRAY_REPO}/releases/download/${version}/${assetName}`;
  logger.log(`- xray: ${XRAY_REPO} ${version} (${assetName})`);

  const tempDir = mkdtempSync(join(tmpdir(), "voyavpn-xray-core-"));
  try {
    const zipFile = join(tempDir, assetName);
    const buffer = await download(url, zipFile, { fetchImpl });
    await verifyChecksum(buffer, `${url}.dgst`, { fetchImpl, logger });

    const extractDir = join(tempDir, "extract");
    extractZip(zipFile, extractDir, { platform, spawn });

    const destinationSeedDir = xraySeedDir(repoRoot);
    const kept = stageExtractedXrayPayload(extractDir, destinationSeedDir, { platform });
    logger.log(`  ✓ staged ${kept.join(", ")} -> ${relative(repoRoot, destinationSeedDir)}/`);

    return { assetName, kept, seedDir: destinationSeedDir, url, version };
  } finally {
    rmSync(tempDir, { force: true, recursive: true });
  }
}

export async function installXrayCore({
  appConfigDir,
  arch = process.arch,
  env = process.env,
  fetchImpl = fetch,
  forceFetch,
  forceInstall = false,
  logger = console,
  platform = process.platform,
  postinstall = false,
  probeExecutable = (path) => probeXrayExecutable(path, { platform }),
  repoRoot,
  spawn = spawnSync,
  stageSeed = fetchAndStageXraySeed,
  version = env.XRAY_VERSION ?? DEFAULT_XRAY_VERSION,
} = {}) {
  const skip = shouldSkipXrayInstall({ env, postinstall });
  if (skip.skip) {
    logger.log(`Skipping Xray install: ${skip.reason}`);
    return { reason: skip.reason, status: "skipped" };
  }

  const resolvedAppConfigDir = appConfigDir ?? defaultAppConfigDir({ env, platform });
  const appExecutable = xrayAppExecutable(resolvedAppConfigDir, platform);
  const effectiveForceFetch = forceFetch ?? truthy(env.VOYAVPN_FORCE_XRAY_FETCH);

  if (!forceInstall && !effectiveForceFetch && probeExecutable(appExecutable)) {
    if (!hasExpectedXrayExecutable(xraySeedDir(repoRoot), platform)) {
      copyXrayAppDataToSeed({
        appConfigDir: resolvedAppConfigDir,
        logger,
        platform,
        repoRoot,
      });
    }
    logger.log(`Xray already installed: ${appExecutable}`);
    return { executable: appExecutable, status: "already-installed" };
  }

  if (effectiveForceFetch || !hasExpectedXrayExecutable(xraySeedDir(repoRoot), platform)) {
    await stageSeed({ arch, fetchImpl, logger, platform, repoRoot, spawn, version });
  }

  const copy = copyXraySeedToAppData({
    appConfigDir: resolvedAppConfigDir,
    logger,
    platform,
    repoRoot,
  });

  if (!copy.copied) {
    throw new Error(`Xray seed was not installed because ${xraySeedDir(repoRoot)} has no ${xrayExecutableName(platform)}`);
  }

  if (!probeExecutable(appExecutable)) {
    throw new Error(`installed Xray executable did not pass -version probe: ${appExecutable}`);
  }

  return { executable: appExecutable, seedDir: copy.sourceDir, status: "installed" };
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
