import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
} from "node:fs";
import { resolve, win32 } from "node:path";

import {
  capture,
  environmentValue,
  commandFailure,
  validateTiming,
  isCliEntrypoint,
  repoRootFromScript,
  run,
} from "../../lib/common.mjs";
import { singBoxExecutableName, singBoxSeedDir } from "../../core/sing-box-installer.mjs";

export const WINDOWS_TUN_SERVICE_NAME = "VoyaVPNTunnelService";
export const WINDOWS_TUN_SERVICE_DISPLAY_NAME = "VoyaVPN Tunnel Service";
export const WINDOWS_TUN_SERVICE_DESCRIPTION =
  "Runs VoyaVPN transparent TUN with sing-box and Wintun.";
export const WINDOWS_TUN_EXIT_STOP_TIMEOUT = 20;
export const WINDOWS_TUN_EXIT_COPY_FAILED = 21;
export const WINDOWS_TUN_EXIT_REGISTRATION_FAILED = 22;

/**
 * Service DACL applied with `sc sdset`. The desktop client is a per-user,
 * non-elevated process (`voya_platform::tun` runs `sc.exe start` without
 * elevation), so Interactive Users need SERVICE_START (RP) and SERVICE_STOP
 * (WP) on top of the query rights the SCM grants them by default. Everything
 * else keeps the SCM defaults: full control for SYSTEM and Administrators,
 * query-only for service logon accounts. Granting stop to interactive users is
 * a deliberate trade: any local process can drop the tunnel, but no caller can
 * choose what the service executes — the core path and the accepted config
 * roots are pinned inside the service binary.
 */
export const WINDOWS_TUN_SERVICE_SDDL =
  "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)"
  + "(A;;CCLCSWRPWPLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)";

const defaultRepoRoot = repoRootFromScript(import.meta.url);
const serviceExecutableName = "voyavpn-tunnel-service.exe";
const productDirName = "VoyaVPN";
const singBoxCoreDirName = "sing_box";
const runtimeStagingDirName = "runtime";
const sleeper = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT));

function defaultWait(milliseconds) {
  Atomics.wait(sleeper, 0, 0, milliseconds);
}

function defaultHashFile(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function missingService(result) {
  return /(?:FAILED\s+)?1060\b/i.test(String(result.stderr || result.stdout || ""));
}

function stoppedService(result) {
  return result.status === 0 && /\bSTOPPED\b/i.test(String(result.stdout || result.stderr || ""));
}

function stoppingService(result) {
  return result.status === 0 && /\bSTOP_PENDING\b/i.test(String(result.stdout || result.stderr || ""));
}

function captureSc(captureCommand, args, cwd) {
  const result = captureCommand("sc.exe", args, { cwd, encoding: "utf8" });
  if (result.error) {
    throw result.error;
  }
  return result;
}

function runSc(runCommand, args, cwd) {
  return runCommand("sc.exe", args, { cwd, shell: false });
}

function serviceQuery(captureCommand, cwd) {
  const result = captureSc(captureCommand, ["query", WINDOWS_TUN_SERVICE_NAME], cwd);
  if (result.status === 0) {
    return { exists: true, result };
  }
  if (missingService(result)) {
    return { exists: false, result };
  }
  throw commandFailure("sc.exe", ["query", WINDOWS_TUN_SERVICE_NAME], result);
}

function stopService({ captureCommand, wait, cwd, timeoutMs, pollIntervalMs }) {
  const initial = serviceQuery(captureCommand, cwd);
  if (!initial.exists || stoppedService(initial.result)) {
    return initial.exists;
  }

  if (!stoppingService(initial.result)) {
    const stopResult = captureSc(captureCommand, ["stop", WINDOWS_TUN_SERVICE_NAME], cwd);
    if (missingService(stopResult)) {
      return false;
    }
    if (stopResult.status !== 0 && !/\b1062\b/.test(String(stopResult.stdout || stopResult.stderr || ""))) {
      throw commandFailure("sc.exe", ["stop", WINDOWS_TUN_SERVICE_NAME], stopResult);
    }
  }

  let elapsedMs = 0;
  while (elapsedMs <= timeoutMs) {
    const current = serviceQuery(captureCommand, cwd);
    if (!current.exists || stoppedService(current.result)) {
      return current.exists;
    }
    if (elapsedMs === timeoutMs) {
      break;
    }
    const delayMs = Math.min(pollIntervalMs, timeoutMs - elapsedMs);
    wait(delayMs);
    elapsedMs += delayMs;
  }

  throw new Error(
    `Timed out after ${timeoutMs}ms waiting for ${WINDOWS_TUN_SERVICE_NAME} to stop. Disable TUN and retry.`,
  );
}

export function requireWindows(platform = process.platform) {
  if (platform !== "win32") {
    throw new Error("Windows tunnel service commands must run on Windows.");
  }
}

export function tunnelServiceSourcePath(repoRoot = defaultRepoRoot, env = process.env) {
  const rustTarget = environmentValue(env, "CARGO_BUILD_TARGET");
  return rustTarget
    ? resolve(repoRoot, "target", rustTarget, "release", serviceExecutableName)
    : resolve(repoRoot, "target", "release", serviceExecutableName);
}

function managedProductDir(env) {
  const programFiles = environmentValue(env, "ProgramW6432", "ProgramFiles");
  if (!programFiles) {
    throw new Error("ProgramFiles is required to install the Windows tunnel service.");
  }
  return win32.join(programFiles, productDirName);
}

export function managedTunnelServicePath(env = process.env) {
  return win32.join(managedProductDir(env), serviceExecutableName);
}

/**
 * The service resolves its sing-box from `<service dir>\sing_box`, never from
 * the caller-supplied config path, so the core must be staged into the same
 * administrator-owned directory as the service binary.
 */
export function managedSingBoxPath(env = process.env) {
  return win32.join(managedProductDir(env), singBoxCoreDirName, "sing-box.exe");
}

/** SYSTEM-owned staging root the service copies accepted configs into. */
export function tunnelRuntimeStagingPath(env = process.env) {
  const programData = environmentValue(env, "ProgramData");
  if (!programData) {
    throw new Error("ProgramData is required to prepare the VoyaVPN tunnel runtime directory.");
  }
  return win32.join(programData, productDirName, runtimeStagingDirName);
}

export function singBoxSeedExecutablePath(repoRoot = defaultRepoRoot) {
  return resolve(singBoxSeedDir(repoRoot), singBoxExecutableName("win32"));
}

export function buildTunnelService({
  env = process.env,
  repoRoot = defaultRepoRoot,
  runCommand = run,
} = {}) {
  runCommand("cargo", ["build", "-p", "voyavpn", "--bin", "voyavpn-tunnel-service", "--release"], {
    cwd: repoRoot,
    env,
    shell: false,
  });
  return tunnelServiceSourcePath(repoRoot, env);
}

function stageProtectedFile({
  sourcePath,
  destinationPath,
  label,
  copyFile,
  fileExists,
  hashFile,
  makeDirectory,
}) {
  try {
    makeDirectory(win32.dirname(destinationPath), { recursive: true });
    copyFile(sourcePath, destinationPath);
  } catch (error) {
    throw new Error(
      `Unable to copy ${label} into the protected location ${destinationPath}. Approve the UAC prompt or run this helper from an elevated terminal.`,
      { cause: error },
    );
  }
  if (!fileExists(destinationPath) || hashFile(sourcePath) !== hashFile(destinationPath)) {
    throw new Error(`${label} copy verification failed: ${destinationPath}`);
  }
}

/**
 * Removes inherited access from the runtime staging root so only SYSTEM and
 * Administrators can write the configs the service hands to sing-box.
 */
function lockRuntimeStagingDirectory(runCommand, runtimeDir, cwd) {
  runCommand(
    "icacls.exe",
    [
      runtimeDir,
      "/inheritance:r",
      "/grant",
      "*S-1-5-18:(OI)(CI)F",
      "/grant",
      "*S-1-5-32-544:(OI)(CI)F",
    ],
    { cwd, shell: false },
  );
}

export function installTunnelService({
  platform = process.platform,
  env = process.env,
  repoRoot = defaultRepoRoot,
  captureCommand = capture,
  runCommand = run,
  fileExists = existsSync,
  fileStat = statSync,
  makeDirectory = mkdirSync,
  copyFile = copyFileSync,
  hashFile = defaultHashFile,
  wait = defaultWait,
  ensureBuilt = buildTunnelService,
  singBoxSourcePath = singBoxSeedExecutablePath(repoRoot),
  timeoutMs = 20_000,
  pollIntervalMs = 250,
} = {}) {
  requireWindows(platform);
  validateTiming(timeoutMs, pollIntervalMs);

  const sourcePath = tunnelServiceSourcePath(repoRoot, env);
  const destinationPath = managedTunnelServicePath(env);
  const singBoxDestinationPath = managedSingBoxPath(env);
  const runtimeStagingDir = tunnelRuntimeStagingPath(env);
  if (!fileExists(sourcePath)) {
    ensureBuilt({ env, repoRoot, runCommand });
  }
  if (!fileExists(sourcePath) || !fileStat(sourcePath).isFile()) {
    throw new Error(`Windows tunnel service build output is missing: ${sourcePath}`);
  }
  if (!fileExists(singBoxSourcePath) || !fileStat(singBoxSourcePath).isFile()) {
    throw new Error(
      `The sing-box core seed is missing: ${singBoxSourcePath}. Run pnpm core:sing-box:install and retry.`,
    );
  }

  const serviceExisted = stopService({
    captureCommand,
    wait,
    cwd: repoRoot,
    timeoutMs,
    pollIntervalMs,
  });

  stageProtectedFile({
    sourcePath,
    destinationPath,
    label: "the Windows tunnel service",
    copyFile,
    fileExists,
    hashFile,
    makeDirectory,
  });
  // The service only ever launches this copy, so the core has to live beside
  // the service binary instead of in the user-writable app-data tree.
  stageProtectedFile({
    sourcePath: singBoxSourcePath,
    destinationPath: singBoxDestinationPath,
    label: "the sing-box core",
    copyFile,
    fileExists,
    hashFile,
    makeDirectory,
  });

  makeDirectory(runtimeStagingDir, { recursive: true });
  lockRuntimeStagingDirectory(runCommand, runtimeStagingDir, repoRoot);

  const quotedDestination = `"${destinationPath}"`;
  const configureArgs = [
    serviceExisted ? "config" : "create",
    WINDOWS_TUN_SERVICE_NAME,
    "binPath=",
    quotedDestination,
    "start=",
    "demand",
    "DisplayName=",
    WINDOWS_TUN_SERVICE_DISPLAY_NAME,
  ];
  runSc(runCommand, configureArgs, repoRoot);
  runSc(
    runCommand,
    ["description", WINDOWS_TUN_SERVICE_NAME, WINDOWS_TUN_SERVICE_DESCRIPTION],
    repoRoot,
  );
  runSc(runCommand, ["sdset", WINDOWS_TUN_SERVICE_NAME, WINDOWS_TUN_SERVICE_SDDL], repoRoot);

  const config = captureSc(captureCommand, ["qc", WINDOWS_TUN_SERVICE_NAME], repoRoot);
  if (config.status !== 0) {
    throw commandFailure("sc.exe", ["qc", WINDOWS_TUN_SERVICE_NAME], config);
  }
  if (!String(config.stdout || config.stderr || "").toLowerCase().includes(destinationPath.toLowerCase())) {
    throw new Error(
      `${WINDOWS_TUN_SERVICE_NAME} was registered with an unexpected binary path. Expected ${destinationPath}.`,
    );
  }

  const installed = serviceQuery(captureCommand, repoRoot);
  if (!installed.exists || !stoppedService(installed.result)) {
    throw new Error(`${WINDOWS_TUN_SERVICE_NAME} must be installed in the stopped state.`);
  }

  return {
    destinationPath,
    serviceExisted,
    sourcePath,
    singBoxDestinationPath,
    singBoxSourcePath,
    runtimeStagingDir,
  };
}

export function uninstallTunnelService({
  platform = process.platform,
  env = process.env,
  repoRoot = defaultRepoRoot,
  captureCommand = capture,
  runCommand = run,
  fileExists = existsSync,
  removeFile = rmSync,
  wait = defaultWait,
  timeoutMs = 20_000,
  pollIntervalMs = 250,
} = {}) {
  requireWindows(platform);
  validateTiming(timeoutMs, pollIntervalMs);
  const destinationPath = managedTunnelServicePath(env);
  const singBoxDestinationPath = managedSingBoxPath(env);
  const existed = stopService({
    captureCommand,
    wait,
    cwd: repoRoot,
    timeoutMs,
    pollIntervalMs,
  });

  if (existed) {
    runSc(runCommand, ["delete", WINDOWS_TUN_SERVICE_NAME], repoRoot);
    let elapsedMs = 0;
    while (elapsedMs <= timeoutMs) {
      if (!serviceQuery(captureCommand, repoRoot).exists) {
        break;
      }
      if (elapsedMs === timeoutMs) {
        throw new Error(
          `Timed out after ${timeoutMs}ms waiting for ${WINDOWS_TUN_SERVICE_NAME} to be deleted.`,
        );
      }
      const delayMs = Math.min(pollIntervalMs, timeoutMs - elapsedMs);
      wait(delayMs);
      elapsedMs += delayMs;
    }
  }

  // Remove only the files this helper staged; the containing directory may
  // hold other VoyaVPN artifacts.
  for (const managed of [destinationPath, singBoxDestinationPath]) {
    if (fileExists(managed)) {
      removeFile(managed, { force: true });
    }
  }
  return { destinationPath, singBoxDestinationPath, serviceExisted: existed };
}

export function queryTunnelService({
  platform = process.platform,
  repoRoot = defaultRepoRoot,
  runCommand = run,
} = {}) {
  requireWindows(platform);
  runSc(runCommand, ["query", WINDOWS_TUN_SERVICE_NAME], repoRoot);
  runSc(runCommand, ["qc", WINDOWS_TUN_SERVICE_NAME], repoRoot);
}

export function tunnelServiceHelp(logger = console) {
  logger.log("usage: node scripts/native/windows/tunnel-service.mjs <build|install|uninstall|status>");
  logger.log("  install/uninstall must run from an elevated Windows terminal.");
}

export function tunnelServiceErrorExitCode(error) {
  const message = error instanceof Error ? error.message : String(error);
  if (/Timed out.*waiting for VoyaVPNTunnelService to stop/i.test(message)) {
    return WINDOWS_TUN_EXIT_STOP_TIMEOUT;
  }
  if (/Unable to copy the |copy verification failed|sing-box core seed is missing/i.test(message)) {
    return WINDOWS_TUN_EXIT_COPY_FAILED;
  }
  if (/sc\.exe|icacls\.exe|registered with an unexpected binary path|installed in the stopped state/i.test(message)) {
    return WINDOWS_TUN_EXIT_REGISTRATION_FAILED;
  }
  return 1;
}

export function runTunnelServiceCommand(command, options = {}) {
  switch (command) {
    case "build":
      return buildTunnelService(options);
    case "install":
      return installTunnelService(options);
    case "uninstall":
      return uninstallTunnelService(options);
    case "status":
      return queryTunnelService(options);
    case "help":
    case "--help":
    case "-h":
      return tunnelServiceHelp(options.logger);
    default:
      throw new Error(`unknown command: ${command}`);
  }
}

export function main(rawArgs = process.argv.slice(2)) {
  const command = rawArgs[0] ?? "help";
  try {
    runTunnelServiceCommand(command);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = tunnelServiceErrorExitCode(error);
  }
}

if (isCliEntrypoint(import.meta.url)) {
  main();
}
