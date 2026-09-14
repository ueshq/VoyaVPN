import { describe, expect, it, vi } from "vitest";

import {
  checkAppUpdate,
  installCheckedAppUpdate,
  loadAppUpdaterStatus,
  type AppUpdateFlowDeps,
} from "@/features/updates/app-update-flow";

describe("app update flow", () => {
  it("loads the signed updater status", async () => {
    const deps = makeDeps({
      appUpdateStatus: vi.fn().mockResolvedValue({
        currentVersion: "1.0.0",
        message: null,
        state: "ready",
      }),
    });

    await expect(loadAppUpdaterStatus(deps)).resolves.toMatchObject({ state: "ready" });
    expect(deps.appUpdateStatus).toHaveBeenCalledTimes(1);
  });

  it("maps an available update to plain UI data and closes the updater resource", async () => {
    const close = vi.fn().mockResolvedValue(undefined);
    const deps = makeDeps({
      checkForAppUpdate: vi.fn().mockResolvedValue(makeTauriUpdate({ close })),
    });

    await expect(checkAppUpdate(deps)).resolves.toEqual({
      currentVersion: "1.0.0",
      update: {
        body: null,
        currentVersion: "1.0.0",
        date: null,
        version: "2.1.0",
      },
    });
    expect(close).toHaveBeenCalledTimes(1);
  });

  it("returns no update when the signed updater has no release", async () => {
    const deps = makeDeps({ checkForAppUpdate: vi.fn().mockResolvedValue(null) });

    await expect(checkAppUpdate(deps)).resolves.toEqual({
      currentVersion: "1.0.0",
      update: null,
    });
  });

  it("installs an available app update and requires restart", async () => {
    const downloadAndInstall = vi.fn().mockResolvedValue(undefined);
    const close = vi.fn().mockResolvedValue(undefined);
    const deps = makeDeps({
      checkForAppUpdate: vi.fn().mockResolvedValue(makeTauriUpdate({ close, downloadAndInstall })),
    });

    await expect(installCheckedAppUpdate(deps)).resolves.toEqual({
      currentVersion: "1.0.0",
      installedVersion: "2.1.0",
      restartRequired: true,
      state: "installed",
    });
    expect(downloadAndInstall).toHaveBeenCalledTimes(1);
    expect(close).toHaveBeenCalledTimes(1);
  });

  it("closes the updater resource while preserving install failures", async () => {
    const close = vi.fn().mockResolvedValue(undefined);
    const deps = makeDeps({
      checkForAppUpdate: vi.fn().mockResolvedValue(
        makeTauriUpdate({
          close,
          downloadAndInstall: vi.fn().mockRejectedValue(new Error("signature invalid")),
        }),
      ),
    });

    await expect(installCheckedAppUpdate(deps)).rejects.toThrow("signature invalid");
    expect(close).toHaveBeenCalledTimes(1);
  });

  it("reports download progress while installing", async () => {
    const downloadAndInstall = vi.fn(async (onEvent: (event: unknown) => void) => {
      onEvent({ data: { contentLength: 200 }, event: "Started" });
      onEvent({ data: { chunkLength: 50 }, event: "Progress" });
      onEvent({ data: { chunkLength: 150 }, event: "Progress" });
      onEvent({ event: "Finished" });
    });
    const deps = makeDeps({
      checkForAppUpdate: vi.fn().mockResolvedValue(makeTauriUpdate({ downloadAndInstall })),
    });
    const progress = vi.fn();

    await installCheckedAppUpdate(deps, progress);

    expect(progress.mock.calls.map(([value]) => value)).toEqual([
      { downloaded: 0, finished: false, total: 200 },
      { downloaded: 50, finished: false, total: 200 },
      { downloaded: 200, finished: false, total: 200 },
      { downloaded: 200, finished: true, total: 200 },
    ]);
  });
});

function makeDeps(overrides: Partial<AppUpdateFlowDeps> = {}): AppUpdateFlowDeps {
  return {
    appUpdateStatus: vi.fn().mockResolvedValue({
      currentVersion: "1.0.0",
      message: null,
      state: "ready",
    }),
    checkForAppUpdate: vi.fn().mockResolvedValue(null),
    getCurrentVersion: vi.fn().mockResolvedValue("1.0.0"),
    ...overrides,
  };
}

function makeTauriUpdate(overrides: Record<string, unknown> = {}) {
  return {
    body: null,
    close: vi.fn().mockResolvedValue(undefined),
    currentVersion: "1.0.0",
    date: null,
    downloadAndInstall: vi.fn().mockResolvedValue(undefined),
    version: "2.1.0",
    ...overrides,
  };
}
