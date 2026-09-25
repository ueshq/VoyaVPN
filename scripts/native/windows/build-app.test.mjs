import { resolve } from "node:path";

import { describe, expect, it, vi } from "vitest";

import {
  assertRustTargetInstalled,
  assertSafeExistingInstalls,
  assertWindowsGuiStopped,
  buildWindowsInstallers,
  buildWindowsLocal,
  discoverWindowsArtifacts,
  elevateTunnelServiceInstall,
  installedWindowsAppPath,
  localWindowsBuildEnv,
  nativeWindowsTarget,
  readExistingWindowsInstalls,
  windowsPnpmInvocation,
} from "./build-app.mjs";

describe("Windows local app build", () => {
  it("maps supported native architectures and rejects unsupported ones", () => {
    expect(nativeWindowsTarget("x64")).toEqual({
      artifactArch: "x64",
      rustTarget: "x86_64-pc-windows-msvc",
    });
    expect(nativeWindowsTarget("arm64")).toEqual({
      artifactArch: "arm64",
      rustTarget: "aarch64-pc-windows-msvc",
    });
    expect(() => nativeWindowsTarget("ia32")).toThrow(/supports only/);
  });

  it("checks that the selected MSVC target library is installed", () => {
    const targetLibDirectory = "C:\\Rust\\msvc\\lib";
    const captureCommand = vi.fn(() => ({
      status: 0,
      stdout: `${targetLibDirectory}\n`,
      stderr: "",
    }));

    expect(assertRustTargetInstalled({
      env: {},
      rustTarget: "x86_64-pc-windows-msvc",
      captureCommand,
      fileExists: (path) => path === targetLibDirectory,
    })).toBe(targetLibDirectory);
    expect(captureCommand).toHaveBeenCalledWith(
      "rustc",
      ["--print", "target-libdir", "--target", "x86_64-pc-windows-msvc"],
      expect.objectContaining({ env: {} }),
    );
  });

  it("fails before building when the selected MSVC target is missing", () => {
    expect(() => assertRustTargetInstalled({
      env: {},
      rustTarget: "x86_64-pc-windows-msvc",
      captureCommand: () => ({
        status: 0,
        stdout: "C:\\Rust\\missing\\lib\n",
        stderr: "",
      }),
      fileExists: () => false,
    })).toThrow(/rustup toolchain install 1\.96\.0-x86_64-pc-windows-msvc.*Visual Studio 2022 Build Tools/);

    const runCommand = vi.fn();
    const ensureGuiStopped = vi.fn();
    expect(() => buildWindowsLocal({
      platform: "win32",
      arch: "x64",
      sourceEnv: { LOCALAPPDATA: "C:\\Users\\dev\\AppData\\Local" },
      repoRoot: "C:\\repo",
      version: "0.1.0",
      runCommand,
      ensureRustTarget: () => {
        throw new Error("Rust MSVC target is missing");
      },
      ensureGuiStopped,
      resolvePackageManager: () => ({ prefixArgs: [], program: "pnpm" }),
    })).toThrow(/Rust MSVC target is missing/);
    expect(runCommand).not.toHaveBeenCalled();
    expect(ensureGuiStopped).not.toHaveBeenCalled();
  });

  it("removes stable updater and signing inputs from the local build environment", () => {
    const env = localWindowsBuildEnv({
      LOCALAPPDATA: "C:\\Users\\dev\\AppData\\Local",
      VOYAVPN_RELEASE_CHANNEL: "stable",
      voyavpn_tauri_updater_config: "stable",
      TAURI_SIGNING_PRIVATE_KEY: "secret",
      WINDOWS_CERTIFICATE_BASE64: "certificate",
      cargo_build_target: "x86_64-pc-windows-gnu",
      KEEP_ME: "yes",
    });

    expect(env).toEqual({
      LOCALAPPDATA: "C:\\Users\\dev\\AppData\\Local",
      KEEP_ME: "yes",
    });
  });

  it("propagates native probe failures instead of treating them as an empty result", () => {
    expect(() => readExistingWindowsInstalls({
      env: {},
      captureCommand: () => ({ status: 1, stdout: "[]", stderr: "Access denied" }),
    })).toThrow(/failed with status 1: Access denied/);

    const spawnError = new Error("Unable to start tasklist");
    expect(() => assertWindowsGuiStopped({
      env: {},
      captureCommand: () => ({ error: spawnError, status: null, stdout: "" }),
    })).toThrow(spawnError);
  });

  it("invokes pnpm through the current Node.js Corepack entrypoint on Windows", () => {
    expect(windowsPnpmInvocation({
      env: {},
      nodeExecutable: "C:\\Node\\node.exe",
      fileExists: (path) => path === "C:\\Node\\node_modules\\corepack\\dist\\pnpm.js",
    })).toEqual({
      prefixArgs: ["C:\\Node\\node_modules\\corepack\\dist\\pnpm.js"],
      program: "C:\\Node\\node.exe",
    });
  });

  it("allows only the expected current-user NSIS installation", () => {
    const appPath = "C:\\Users\\dev\\AppData\\Local\\VoyaVPN\\voyavpn.exe";
    expect(() => assertSafeExistingInstalls([
      {
        scope: "HKCU",
        installLocation: "\"C:\\Users\\dev\\AppData\\Local\\VoyaVPN\"",
        windowsInstaller: false,
      },
    ], appPath)).not.toThrow();

    expect(() => assertSafeExistingInstalls([
      {
        scope: "HKLM",
        installLocation: "C:\\Program Files\\VoyaVPN",
        windowsInstaller: true,
      },
    ], appPath)).toThrow(/Refusing to replace/);
  });

  it("detects a running GUI without relying on localized tasklist text", () => {
    expect(() => assertWindowsGuiStopped({
      env: {},
      captureCommand: () => ({
        status: 0,
        stdout: '"voyavpn.exe","123","Console","1","10,000 K"',
        stderr: "",
      }),
    })).toThrow(/still running/);
  });

  it("parses zero, one, or many registry entries returned by PowerShell", () => {
    expect(readExistingWindowsInstalls({
      env: {},
      captureCommand: () => ({ status: 0, stdout: "[]\n", stderr: "" }),
    })).toEqual([]);
    expect(readExistingWindowsInstalls({
      env: {},
      captureCommand: () => ({
        status: 0,
        stdout: '{"scope":"HKCU","installLocation":"C:\\\\VoyaVPN"}\n',
        stderr: "",
      }),
    })).toEqual([{ scope: "HKCU", installLocation: "C:\\VoyaVPN" }]);
  });

  it("selects only the current-version native-architecture NSIS artifact", () => {
    const readDirectory = vi.fn(() => [
      "VoyaVPN_0.1.0_x64-setup.exe",
      "VoyaVPN_0.1.0_arm64-setup.exe",
    ]);
    const artifacts = discoverWindowsArtifacts({
      repoRoot: "C:\\repo",
      version: "0.1.0",
      artifactArch: "x64",
      rustTarget: "x86_64-pc-windows-msvc",
      fileExists: () => true,
      readDirectory,
      fileStat: () => ({ isFile: () => true }),
    });

    expect(artifacts).toEqual({
      nsis: expect.stringMatching(/VoyaVPN_0\.1\.0_x64-setup\.exe$/),
    });
    expect(readDirectory).toHaveBeenCalledWith(
      resolve("C:\\repo", "target", "x86_64-pc-windows-msvc", "release", "bundle", "nsis"),
    );
  });

  it("builds, silently installs, elevates only the service step, and verifies outputs", () => {
    const sourceEnv = {
      LOCALAPPDATA: "C:\\Users\\dev\\AppData\\Local",
      ProgramFiles: "C:\\Program Files",
      VOYAVPN_RELEASE_CHANNEL: "stable",
      CARGO_BUILD_TARGET: "x86_64-pc-windows-gnu",
      KEEP_ME: "yes",
    };
    const appPath = installedWindowsAppPath(sourceEnv);
    const servicePath = "C:\\Program Files\\VoyaVPN\\voyavpn-tunnel-service.exe";
    const singBoxPath = "C:\\Program Files\\VoyaVPN\\sing_box\\sing-box.exe";
    const nsis = "C:\\repo\\target\\x86_64-pc-windows-msvc\\release\\bundle\\nsis\\VoyaVPN_0.1.0_x64-setup.exe";
    const runCommand = vi.fn();
    const ensureGuiStopped = vi.fn();
    const ensureRustTarget = vi.fn();
    const elevateServiceInstall = vi.fn();
    const discoverArtifacts = vi.fn(() => ({ nsis }));
    const packageManager = { prefixArgs: [], program: "pnpm" };
    const logger = { log: vi.fn() };

    const result = buildWindowsLocal({
      platform: "win32",
      arch: "x64",
      sourceEnv,
      repoRoot: "C:\\repo",
      version: "0.1.0",
      runCommand,
      fileExists: (path) => path === appPath || path === servicePath || path === singBoxPath,
      ensureRustTarget,
      ensureGuiStopped,
      readExistingInstalls: () => [],
      discoverArtifacts,
      elevateServiceInstall,
      resolvePackageManager: () => packageManager,
      logger,
    });

    expect(result).toEqual({ appPath, servicePath, singBoxPath, nsis });
    expect(ensureGuiStopped).toHaveBeenCalledOnce();
    expect(runCommand.mock.calls.map(([program, args]) => [program, args])).toEqual([
      ["pnpm", [
        "tauri:build",
        "--no-sign",
        "--target",
        "x86_64-pc-windows-msvc",
        "--bundles",
        "nsis",
      ]],
      ["pnpm", ["native:windows:tunnel:build"]],
      [nsis, ["/S"]],
      ["sc.exe", ["query", "VoyaVPNTunnelService"]],
    ]);
    const buildEnvironment = runCommand.mock.calls[0][2].env;
    expect(buildEnvironment.KEEP_ME).toBe("yes");
    expect(buildEnvironment.VOYAVPN_RELEASE_CHANNEL).toBeUndefined();
    expect(buildEnvironment.CARGO_BUILD_TARGET).toBe("x86_64-pc-windows-msvc");
    expect(ensureRustTarget).toHaveBeenCalledWith({
      env: buildEnvironment,
      rustTarget: "x86_64-pc-windows-msvc",
    });
    expect(discoverArtifacts).toHaveBeenCalledWith(expect.objectContaining({
      rustTarget: "x86_64-pc-windows-msvc",
    }));
    expect(elevateServiceInstall).toHaveBeenCalledOnce();
  });

  it("does not perform work on non-Windows hosts", () => {
    const runCommand = vi.fn();
    expect(() => buildWindowsLocal({
      platform: "linux",
      sourceEnv: {},
      repoRoot: "/repo",
      version: "0.1.0",
      runCommand,
    })).toThrow(/must run on Windows/);
    expect(runCommand).not.toHaveBeenCalled();
  });

  it("reports UAC cancellation distinctly", () => {
    let powershellScript = "";
    expect(() => elevateTunnelServiceInstall({
      env: {},
      rustTarget: "x86_64-pc-windows-msvc",
      repoRoot: "C:\\repo",
      nodeExecutable: "C:\\Program Files\\nodejs\\node.exe",
      captureCommand: (_program, args) => {
        powershellScript = args.at(-1);
        return { status: 1223 };
      },
    })).toThrow(/cancelled at the UAC prompt/);
    expect(powershellScript).toMatch(/-WindowStyle Hidden/);
    expect(powershellScript).toMatch(/InnerException\.NativeErrorCode/);
  });

  it("maps elevated service failure codes to actionable errors", () => {
    expect(() => elevateTunnelServiceInstall({
      env: { ProgramFiles: "C:\\Program Files" },
      repoRoot: "C:\\repo",
      captureCommand: () => ({ status: 21 }),
    })).toThrow(/protected tunnel service binary.*Program Files permissions/);
    expect(() => elevateTunnelServiceInstall({
      env: { ProgramFiles: "C:\\Program Files" },
      repoRoot: "C:\\repo",
      captureCommand: () => ({ status: 22 }),
    })).toThrow(/registration or verification failed/);
  });

  it("reports NSIS bundling failures with local-lane context", () => {
    expect(() => buildWindowsInstallers({
      env: {},
      rustTarget: "x86_64-pc-windows-msvc",
      repoRoot: "C:\\repo",
      packageManager: { prefixArgs: [], program: "pnpm" },
      runCommand: () => {
        throw new Error("light.exe failed");
      },
    })).toThrow(/Windows NSIS build failed/);
  });
});
