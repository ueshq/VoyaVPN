import { existsSync, readdirSync, statSync } from "node:fs";
import { resolve, win32 } from "node:path";

import {
  capture,
  checkedCapture,
  environmentValue,
  isCliEntrypoint,
  readJson,
  repoRootFromScript,
  run,
} from "../../lib/common.mjs";
import {
  managedSingBoxPath,
  managedTunnelServicePath,
  WINDOWS_TUN_EXIT_COPY_FAILED,
  WINDOWS_TUN_EXIT_REGISTRATION_FAILED,
  WINDOWS_TUN_EXIT_STOP_TIMEOUT,
  WINDOWS_TUN_SERVICE_NAME,
} from "./tunnel-service.mjs";

const defaultRepoRoot = repoRootFromScript(import.meta.url);
const productName = "VoyaVPN";
const mainExecutableName = "voyavpn.exe";

const localBuildEnvNames = [
  "CARGO_BUILD_TARGET",
  "VOYAVPN_RELEASE_CHANNEL",
  "RELEASE_CHANNEL",
  "CHANNEL",
  "VOYAVPN_TAURI_UPDATER_CONFIG",
  "VOYAVPN_UPDATES_BASE_URL",
  "VOYAVPN_UPDATER_PUBLIC_KEY",
  "TAURI_UPDATER_PUBLIC_KEY",
  "TAURI_SIGNING_PRIVATE_KEY",
  "TAURI_SIGNING_PRIVATE_KEY_PATH",
  "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
  "WINDOWS_CERTIFICATE_BASE64",
  "WINDOWS_CERTIFICATE_PASSWORD",
];

function trimWindowsPath(value) {
  const trimmed = String(value ?? "").trim().replace(/^"|"$/g, "");
  return trimmed ? win32.normalize(trimmed).replace(/[\\/]+$/, "") : "";
}

function powershellQuoted(value) {
  return `'${String(value).replaceAll("'", "''")}'`;
}

function installLocationFromEntry(entry) {
  const explicit = trimWindowsPath(entry.installLocation);
  if (explicit) {
    return explicit;
  }
  const uninstall = String(entry.uninstallString ?? "").trim();
  const executable = uninstall.match(/^"([^"]+)"/)?.[1] ?? uninstall.split(/\s+/)[0] ?? "";
  return executable ? win32.dirname(trimWindowsPath(executable)) : "";
}

function powershellExecutable(env) {
  const systemRoot = environmentValue(env, "SystemRoot", "windir");
  return systemRoot
    ? win32.join(systemRoot, "System32", "WindowsPowerShell", "v1.0", "powershell.exe")
    : "powershell.exe";
}

function powershellJsonScript() {
  return [
    "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)",
    "$roots = @(",
    "  @{ Scope = 'HKCU'; Path = 'Registry::HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall' },",
    "  @{ Scope = 'HKLM'; Path = 'Registry::HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall' },",
    "  @{ Scope = 'HKLM'; Path = 'Registry::HKEY_LOCAL_MACHINE\\Software\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall' }",
    ")",
    "$entries = @($roots | ForEach-Object {",
    "  $scope = $_.Scope",
    "  Get-ChildItem -LiteralPath $_.Path -ErrorAction SilentlyContinue | ForEach-Object {",
    "    $item = Get-ItemProperty -LiteralPath $_.PSPath -ErrorAction SilentlyContinue",
    "    if ($item.DisplayName -eq 'VoyaVPN') {",
    "      [pscustomobject]@{",
    "        scope = $scope",
    "        key = $_.PSChildName",
    "        installLocation = [string]$item.InstallLocation",
    "        uninstallString = [string]$item.UninstallString",
    "        windowsInstaller = ([int]$item.WindowsInstaller -eq 1)",
    "      }",
    "    }",
    "  }",
    "})",
    "Write-Output (ConvertTo-Json -Compress -InputObject $entries)",
  ].join("\n");
}

export function nativeWindowsTarget(arch = process.arch) {
  if (arch === "x64") {
    return { artifactArch: "x64", rustTarget: "x86_64-pc-windows-msvc" };
  }
  if (arch === "arm64") {
    return { artifactArch: "arm64", rustTarget: "aarch64-pc-windows-msvc" };
  }
  throw new Error(`pnpm build:windows:local supports only native Windows x64 and arm64, not ${arch}.`);
}

function rustMsvcPrerequisiteMessage(rustTarget) {
  const toolchain = `1.96.0-${rustTarget}`;
  return [
    `Rust MSVC target ${rustTarget} is not installed for the active toolchain.`,
    `Install the matching toolchain with \`rustup toolchain install ${toolchain}\``,
    `and select it for this repository with \`rustup override set ${toolchain}\`.`,
    "Visual Studio 2022 Build Tools must also include Desktop development with C++, MSVC v143, and a Windows 10/11 SDK.",
  ].join(" ");
}

export function assertRustTargetInstalled({
  env = process.env,
  rustTarget = nativeWindowsTarget().rustTarget,
  captureCommand = capture,
  fileExists = existsSync,
} = {}) {
  let result;
  try {
    result = checkedCapture(
      "rustc",
      ["--print", "target-libdir", "--target", rustTarget],
      { env, encoding: "utf8", windowsHide: true },
      captureCommand,
    );
  } catch (error) {
    throw new Error(rustMsvcPrerequisiteMessage(rustTarget), { cause: error });
  }
  const targetLibDirectory = String(result.stdout ?? "").trim();
  if (!targetLibDirectory || !fileExists(targetLibDirectory)) {
    throw new Error(rustMsvcPrerequisiteMessage(rustTarget));
  }
  return targetLibDirectory;
}

export function localWindowsBuildEnv(sourceEnv = process.env) {
  const env = { ...sourceEnv };
  const blocked = new Set(localBuildEnvNames.map((name) => name.toLowerCase()));
  for (const name of Object.keys(env)) {
    if (blocked.has(name.toLowerCase())) {
      delete env[name];
    }
  }
  return env;
}

export function windowsPnpmInvocation({
  env = process.env,
  nodeExecutable = process.execPath,
  fileExists = existsSync,
} = {}) {
  const npmExecPath = environmentValue(env, "npm_execpath");
  const bundledCorepack = win32.join(
    win32.dirname(nodeExecutable),
    "node_modules",
    "corepack",
    "dist",
    "pnpm.js",
  );
  const cliPath = [npmExecPath, bundledCorepack]
    .filter((path) => /\.[cm]?js$/i.test(path))
    .find((path) => fileExists(path));
  if (!cliPath) {
    throw new Error(
      "Unable to locate Corepack's pnpm.js beside the current Node.js executable. Install Node.js with Corepack and retry.",
    );
  }
  return { prefixArgs: [cliPath], program: nodeExecutable };
}

export function installedWindowsAppPath(env = process.env) {
  const localAppData = environmentValue(env, "LOCALAPPDATA");
  if (!localAppData) {
    throw new Error("LOCALAPPDATA is required to install the local Windows client.");
  }
  return win32.join(localAppData, productName, mainExecutableName);
}

export function assertSafeExistingInstalls(entries, expectedAppPath) {
  const expectedDirectory = trimWindowsPath(win32.dirname(expectedAppPath));
  for (const entry of entries ?? []) {
    const location = installLocationFromEntry(entry);
    const isExpectedNsis =
      String(entry.scope).toUpperCase() === "HKCU"
      && !entry.windowsInstaller
      && location.toLowerCase() === expectedDirectory.toLowerCase();
    if (!isExpectedNsis) {
      const label = location || entry.key || "unknown location";
      throw new Error(
        `Refusing to replace an existing VoyaVPN MSI or non-local installation at ${label}. Uninstall it explicitly before running pnpm build:windows:local.`,
      );
    }
  }
}

export function readExistingWindowsInstalls({ env = process.env, captureCommand = capture } = {}) {
  const executable = powershellExecutable(env);
  const args = ["-NoProfile", "-NonInteractive", "-Command", powershellJsonScript()];
  const result = checkedCapture(executable, args, {
    env,
    encoding: "utf8",
    windowsHide: true,
  }, captureCommand);
  const output = String(result.stdout ?? "").trim();
  if (!output) {
    return [];
  }
  try {
    const parsed = JSON.parse(output);
    return Array.isArray(parsed) ? parsed : [parsed];
  } catch (error) {
    throw new Error(`Unable to parse installed VoyaVPN registry entries: ${output}`, { cause: error });
  }
}

export function assertWindowsGuiStopped({ env = process.env, captureCommand = capture } = {}) {
  const result = checkedCapture(
    "tasklist.exe",
    ["/FI", `IMAGENAME eq ${mainExecutableName}`, "/NH", "/FO", "CSV"],
    { env, encoding: "utf8", windowsHide: true },
    captureCommand,
  );
  if (String(result.stdout ?? "").split(/\r?\n/).some((line) => /^"voyavpn\.exe"/i.test(line.trim()))) {
    throw new Error(
      "VoyaVPN is still running. Quit the app and disable TUN before pnpm build:windows:local replaces it.",
    );
  }
}

function currentVersion(repoRoot) {
  const manifest = readJson(resolve(repoRoot, "package.json"));
  return String(manifest.version);
}

function artifactFiles(directory, fileExists, readDirectory, fileStat) {
  if (!fileExists(directory)) {
    return [];
  }
  return readDirectory(directory)
    .map((name) => resolve(directory, name))
    .filter((path) => fileStat(path).isFile());
}

function selectArtifact(paths, { version, artifactArch, extension, label }) {
  const prefix = `_${version}_`;
  const matches = paths.filter((path) => {
    const name = win32.basename(path).toLowerCase();
    return name.endsWith(extension)
      && name.includes(prefix.toLowerCase())
      && name.includes(`_${artifactArch.toLowerCase()}`);
  });
  if (matches.length !== 1) {
    throw new Error(
      `Expected exactly one ${label} artifact for VoyaVPN ${version} ${artifactArch}, found ${matches.length}. Remove stale matching bundles and retry.`,
    );
  }
  return matches[0];
}

export function discoverWindowsArtifacts({
  repoRoot = defaultRepoRoot,
  version = currentVersion(repoRoot),
  artifactArch = nativeWindowsTarget().artifactArch,
  rustTarget = nativeWindowsTarget().rustTarget,
  fileExists = existsSync,
  readDirectory = readdirSync,
  fileStat = statSync,
} = {}) {
  const bundleRoot = resolve(repoRoot, "target", rustTarget, "release", "bundle");
  const nsis = selectArtifact(
    artifactFiles(resolve(bundleRoot, "nsis"), fileExists, readDirectory, fileStat),
    { version, artifactArch, extension: ".exe", label: "NSIS" },
  );
  return { nsis };
}

export function elevateTunnelServiceInstall({
  env = process.env,
  repoRoot = defaultRepoRoot,
  nodeExecutable = process.execPath,
  captureCommand = capture,
} = {}) {
  const helperPath = resolve(repoRoot, "scripts", "native", "windows", "tunnel-service.mjs");
  const elevatedEnv = {
    ...env,
    VOYAVPN_ELEVATED_NODE: nodeExecutable,
    VOYAVPN_ELEVATED_SCRIPT: helperPath,
  };
  const script = [
    "$ErrorActionPreference = 'Stop'",
    "$quotedScript = [char]34 + $env:VOYAVPN_ELEVATED_SCRIPT + [char]34",
    "try {",
    "  $child = Start-Process -FilePath $env:VOYAVPN_ELEVATED_NODE -ArgumentList @($quotedScript, 'install') -Verb RunAs -WindowStyle Hidden -Wait -PassThru",
    "  exit $child.ExitCode",
    "} catch {",
    "  [Console]::Error.WriteLine($_.Exception.Message)",
    "  $nativeCode = $_.Exception.NativeErrorCode",
    "  if (-not $nativeCode -and $_.Exception.InnerException) {",
    "    $nativeCode = $_.Exception.InnerException.NativeErrorCode",
    "  }",
    "  if ($nativeCode -eq 1223) { exit 1223 }",
    "  exit 1",
    "}",
  ].join("\n");
  const executable = powershellExecutable(env);
  const args = ["-NoProfile", "-NonInteractive", "-Command", script];
  const result = captureCommand(executable, args, {
    cwd: repoRoot,
    env: elevatedEnv,
    stdio: "inherit",
    windowsHide: true,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    const messages = new Map([
      [1223, () => "Windows service installation was cancelled at the UAC prompt."],
      [WINDOWS_TUN_EXIT_STOP_TIMEOUT, () => "Timed out waiting for VoyaVPNTunnelService to stop. Disable TUN and retry."],
      [WINDOWS_TUN_EXIT_COPY_FAILED, () => `Unable to stage the protected tunnel service binary at ${managedTunnelServicePath(env)} or the sing-box core at ${managedSingBoxPath(env)}. Run pnpm core:sing-box:install, then check Program Files permissions and antivirus logs.`],
      [WINDOWS_TUN_EXIT_REGISTRATION_FAILED, () => "VoyaVPNTunnelService registration or verification failed. Inspect sc.exe query and sc.exe qc output, then retry."],
    ]);
    const message = messages.get(result.status);
    throw new Error(
      message?.()
        ?? `Elevated Windows service installation failed with status ${result.status ?? "unknown"}.`,
    );
  }
}

export function buildWindowsInstallers({
  env,
  rustTarget,
  repoRoot = defaultRepoRoot,
  runCommand = run,
  packageManager = windowsPnpmInvocation({ env }),
} = {}) {
  try {
    return runCommand(
      packageManager.program,
      [
        ...packageManager.prefixArgs,
        "tauri:build",
        "--no-sign",
        "--target",
        rustTarget,
        "--bundles",
        "nsis",
      ],
      {
        cwd: repoRoot,
        env,
        shell: false,
      },
    );
  } catch (error) {
    throw new Error(
      "Windows NSIS build failed. Inspect the Tauri bundler output above for the underlying error.",
      { cause: error },
    );
  }
}

export function buildWindowsLocal({
  platform = process.platform,
  arch = process.arch,
  sourceEnv = process.env,
  repoRoot = defaultRepoRoot,
  version = currentVersion(repoRoot),
  runCommand = run,
  fileExists = existsSync,
  ensureGuiStopped = assertWindowsGuiStopped,
  readExistingInstalls = readExistingWindowsInstalls,
  discoverArtifacts = discoverWindowsArtifacts,
  elevateServiceInstall = elevateTunnelServiceInstall,
  ensureRustTarget = assertRustTargetInstalled,
  resolvePackageManager = windowsPnpmInvocation,
  logger = console,
} = {}) {
  if (platform !== "win32") {
    throw new Error("pnpm build:windows:local must run on Windows.");
  }
  const target = nativeWindowsTarget(arch);
  const env = {
    ...localWindowsBuildEnv(sourceEnv),
    CARGO_BUILD_TARGET: target.rustTarget,
  };
  const appPath = installedWindowsAppPath(env);
  const packageManager = resolvePackageManager({ env });

  ensureRustTarget({ env, rustTarget: target.rustTarget });
  ensureGuiStopped({ env });
  assertSafeExistingInstalls(readExistingInstalls({ env }), appPath);

  logger.log(`Building unsigned local Windows ${target.artifactArch} NSIS installer and TUN service...`);
  buildWindowsInstallers({
    env,
    rustTarget: target.rustTarget,
    repoRoot,
    runCommand,
    packageManager,
  });
  runCommand(packageManager.program, [...packageManager.prefixArgs, "native:windows:tunnel:build"], {
    cwd: repoRoot,
    env,
    shell: false,
  });

  const artifacts = discoverArtifacts({
    repoRoot,
    version,
    artifactArch: target.artifactArch,
    rustTarget: target.rustTarget,
  });
  logger.log(`Installing ${artifacts.nsis} for the current user...`);
  runCommand(artifacts.nsis, ["/S"], {
    cwd: win32.dirname(artifacts.nsis),
    env,
    shell: false,
  });
  if (!fileExists(appPath)) {
    throw new Error(`NSIS completed but the installed client is missing: ${appPath}`);
  }

  logger.log("Requesting administrator permission to update the Windows tunnel service...");
  elevateServiceInstall({ env, repoRoot });
  const servicePath = managedTunnelServicePath(env);
  if (!fileExists(servicePath)) {
    throw new Error(`The elevated installer completed but the tunnel service binary is missing: ${servicePath}`);
  }
  // The service refuses to launch a core outside its own directory, so a
  // missing managed core means TUN would fail at connect time instead of here.
  const singBoxPath = managedSingBoxPath(env);
  if (!fileExists(singBoxPath)) {
    throw new Error(`The elevated installer completed but the managed sing-box core is missing: ${singBoxPath}`);
  }
  runCommand("sc.exe", ["query", WINDOWS_TUN_SERVICE_NAME], {
    cwd: repoRoot,
    env,
    shell: false,
  });

  logger.log("");
  logger.log("Local Windows TUN test app is ready:");
  logger.log(`  Client: ${appPath}`);
  logger.log(`  Service: ${servicePath}`);
  logger.log(`  Service core: ${singBoxPath}`);
  logger.log(`  NSIS: ${artifacts.nsis}`);
  logger.log("");
  logger.log("Open it with:");
  logger.log(`  Start-Process ${powershellQuoted(appPath)}`);
  logger.log("");
  logger.log("This build is unsigned and intended only for local testing. Do not distribute it.");
  logger.log("Do not use pnpm dev for Windows TUN testing; it does not ensure the service is installed.");

  return { appPath, servicePath, singBoxPath, ...artifacts };
}

export function main() {
  try {
    buildWindowsLocal();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

if (isCliEntrypoint(import.meta.url)) {
  main();
}
