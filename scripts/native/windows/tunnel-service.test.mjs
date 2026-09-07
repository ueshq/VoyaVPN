import { describe, expect, it, vi } from "vitest";

import {
  installTunnelService,
  managedSingBoxPath,
  managedTunnelServicePath,
  tunnelRuntimeStagingPath,
  tunnelServiceErrorExitCode,
  tunnelServiceSourcePath,
  uninstallTunnelService,
  WINDOWS_TUN_SERVICE_NAME,
  WINDOWS_TUN_SERVICE_SDDL,
} from "./tunnel-service.mjs";

const missingService = { status: 1, stdout: "[SC] OpenService FAILED 1060", stderr: "" };
const runningService = { status: 0, stdout: "STATE              : 4  RUNNING", stderr: "" };
const stoppingService = { status: 0, stdout: "STATE              : 3  STOP_PENDING", stderr: "" };
const stoppedService = { status: 0, stdout: "STATE              : 1  STOPPED", stderr: "" };

const env = { ProgramFiles: "C:\\Program Files", ProgramData: "C:\\ProgramData" };

function installFixture(queryResults) {
  const repoRoot = "/repo";
  const sourcePath = tunnelServiceSourcePath(repoRoot);
  const singBoxSourcePath = "/repo/seed/sing-box.exe";
  const destinationPath = managedTunnelServicePath(env);
  const singBoxDestinationPath = managedSingBoxPath(env);
  const files = new Set([sourcePath, singBoxSourcePath]);
  const runCommand = vi.fn();
  const copyFile = vi.fn((source, destination) => {
    expect([sourcePath, singBoxSourcePath]).toContain(source);
    files.add(destination);
  });
  const captureCommand = vi.fn((_program, args) => {
    if (args[0] === "query") {
      return queryResults.shift() ?? stoppedService;
    }
    if (args[0] === "stop") {
      return { status: 0, stdout: "STOP_PENDING", stderr: "" };
    }
    if (args[0] === "qc") {
      return { status: 0, stdout: `BINARY_PATH_NAME   : "${destinationPath}"`, stderr: "" };
    }
    throw new Error(`unexpected sc command: ${args.join(" ")}`);
  });
  const makeDirectory = vi.fn();

  return {
    options: {
      platform: "win32",
      env,
      repoRoot,
      captureCommand,
      runCommand,
      fileExists: (path) => files.has(path),
      fileStat: () => ({ isFile: () => true }),
      makeDirectory,
      copyFile,
      hashFile: () => "same-hash",
      wait: vi.fn(),
      singBoxSourcePath,
    },
    destinationPath,
    singBoxDestinationPath,
    singBoxSourcePath,
    makeDirectory,
    runCommand,
  };
}

describe("Windows tunnel service helper", () => {
  it("uses the native Program Files directory for the managed service binary", () => {
    expect(managedTunnelServicePath({ ProgramW6432: "D:\\Program Files" })).toBe(
      "D:\\Program Files\\VoyaVPN\\voyavpn-tunnel-service.exe",
    );
  });

  it("installs a new demand-start service with a quoted protected binary path", () => {
    const fixture = installFixture([missingService, stoppedService]);

    const result = installTunnelService(fixture.options);

    expect(result).toMatchObject({
      destinationPath: fixture.destinationPath,
      serviceExisted: false,
    });
    expect(fixture.runCommand).toHaveBeenCalledWith(
      "sc.exe",
      [
        "create",
        WINDOWS_TUN_SERVICE_NAME,
        "binPath=",
        `"${fixture.destinationPath}"`,
        "start=",
        "demand",
        "DisplayName=",
        "VoyaVPN Tunnel Service",
      ],
      { cwd: "/repo", shell: false },
    );
  });

  it("stops and reconfigures an existing service without starting it", () => {
    const fixture = installFixture([runningService, stoppedService, stoppedService]);

    const result = installTunnelService(fixture.options);

    expect(result.serviceExisted).toBe(true);
    expect(fixture.options.captureCommand).toHaveBeenCalledWith(
      "sc.exe",
      ["stop", WINDOWS_TUN_SERVICE_NAME],
      expect.any(Object),
    );
    expect(fixture.runCommand).toHaveBeenCalledWith(
      "sc.exe",
      expect.arrayContaining(["config", WINDOWS_TUN_SERVICE_NAME, "start=", "demand"]),
      expect.any(Object),
    );
    expect(fixture.runCommand).not.toHaveBeenCalledWith(
      "sc.exe",
      expect.arrayContaining(["start", WINDOWS_TUN_SERVICE_NAME]),
      expect.any(Object),
    );
  });

  it("waits for an already stopping service without sending a second stop", () => {
    const fixture = installFixture([stoppingService, stoppedService, stoppedService]);

    installTunnelService(fixture.options);

    expect(fixture.options.captureCommand).not.toHaveBeenCalledWith(
      "sc.exe",
      ["stop", WINDOWS_TUN_SERVICE_NAME],
      expect.any(Object),
    );
  });

  it("uninstalls idempotently and removes only the managed executables", () => {
    const destinationPath = managedTunnelServicePath(env);
    const singBoxDestinationPath = managedSingBoxPath(env);
    const staged = new Set([destinationPath, singBoxDestinationPath]);
    const removeFile = vi.fn();
    const runCommand = vi.fn();
    const captureCommand = vi.fn()
      .mockReturnValueOnce(stoppedService)
      .mockReturnValueOnce(missingService);

    const result = uninstallTunnelService({
      platform: "win32",
      env,
      repoRoot: "/repo",
      captureCommand,
      runCommand,
      fileExists: (path) => staged.has(path),
      removeFile,
      wait: vi.fn(),
    });

    expect(result.serviceExisted).toBe(true);
    expect(runCommand).toHaveBeenCalledWith(
      "sc.exe",
      ["delete", WINDOWS_TUN_SERVICE_NAME],
      { cwd: "/repo", shell: false },
    );
    expect(removeFile).toHaveBeenCalledWith(destinationPath, { force: true });
    expect(removeFile).toHaveBeenCalledWith(singBoxDestinationPath, { force: true });
  });

  it("reports a service stop timeout before replacing the executable", () => {
    const fixture = installFixture([runningService, runningService]);

    expect(() => installTunnelService({
      ...fixture.options,
      timeoutMs: 0,
    })).toThrow(/Timed out.*waiting for VoyaVPNTunnelService to stop/);
    expect(fixture.options.copyFile).not.toHaveBeenCalled();
  });

  it("explains protected-path copy failures", () => {
    const fixture = installFixture([missingService]);
    fixture.options.copyFile = () => {
      throw new Error("access denied");
    };

    expect(() => installTunnelService(fixture.options)).toThrow(/protected location.*UAC prompt/);
  });

  it("classifies elevated installation failures for the parent process", () => {
    expect(tunnelServiceErrorExitCode(new Error(
      "Timed out after 20000ms waiting for VoyaVPNTunnelService to stop.",
    ))).toBe(20);
    expect(tunnelServiceErrorExitCode(new Error(
      "the Windows tunnel service copy verification failed.",
    ))).toBe(21);
    expect(tunnelServiceErrorExitCode(new Error(
      "The sing-box core seed is missing: /repo/seed/sing-box.exe.",
    ))).toBe(21);
    expect(tunnelServiceErrorExitCode(new Error(
      "sc.exe config VoyaVPNTunnelService failed.",
    ))).toBe(22);
  });

  it("stages the sing-box core beside the service so the caller cannot pick the binary", () => {
    const fixture = installFixture([missingService, stoppedService]);

    const result = installTunnelService(fixture.options);

    expect(result.singBoxDestinationPath).toBe(
      "C:\\Program Files\\VoyaVPN\\sing_box\\sing-box.exe",
    );
    expect(fixture.options.copyFile).toHaveBeenCalledWith(
      fixture.singBoxSourcePath,
      fixture.singBoxDestinationPath,
    );
  });

  it("refuses to install without a staged sing-box seed", () => {
    const fixture = installFixture([missingService, stoppedService]);
    fixture.options.fileExists = (path) => path === tunnelServiceSourcePath("/repo");

    expect(() => installTunnelService(fixture.options)).toThrow(/sing-box core seed is missing/);
    expect(fixture.options.copyFile).not.toHaveBeenCalled();
  });

  it("locks the runtime staging root to SYSTEM and Administrators", () => {
    const fixture = installFixture([missingService, stoppedService]);

    const result = installTunnelService(fixture.options);

    expect(result.runtimeStagingDir).toBe(tunnelRuntimeStagingPath(env));
    expect(fixture.makeDirectory).toHaveBeenCalledWith(result.runtimeStagingDir, {
      recursive: true,
    });
    expect(fixture.runCommand).toHaveBeenCalledWith(
      "icacls.exe",
      [
        result.runtimeStagingDir,
        "/inheritance:r",
        "/grant",
        "*S-1-5-18:(OI)(CI)F",
        "/grant",
        "*S-1-5-32-544:(OI)(CI)F",
      ],
      { cwd: "/repo", shell: false },
    );
  });

  it("grants interactive users start and stop through an explicit service DACL", () => {
    const fixture = installFixture([missingService, stoppedService]);

    installTunnelService(fixture.options);

    expect(fixture.runCommand).toHaveBeenCalledWith(
      "sc.exe",
      ["sdset", WINDOWS_TUN_SERVICE_NAME, WINDOWS_TUN_SERVICE_SDDL],
      { cwd: "/repo", shell: false },
    );
    expect(WINDOWS_TUN_SERVICE_SDDL).toContain("(A;;CCLCSWRPWPLOCRRC;;;IU)");
  });

  it("rejects service installation outside Windows before changing anything", () => {
    expect(() => installTunnelService({ platform: "linux" })).toThrow(/must run on Windows/);
  });
});
