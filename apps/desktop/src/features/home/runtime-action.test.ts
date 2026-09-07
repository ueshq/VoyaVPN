import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AppError } from "@/ipc/bindings";

const ipcMocks = vi.hoisted(() => {
  // A faithful stand-in for the real error: `runWithElevation` and
  // `missingCorePayload` both branch on `appError.kind`, so a bare
  // `class extends Error {}` would make every branch below unreachable.
  class MockIpcCommandError extends Error {
    readonly appError: AppError;

    constructor(appError: AppError, message = "ipc failed") {
      super(message);
      this.appError = appError;
      this.name = "IpcCommandError";
    }
  }

  return { IpcCommandError: MockIpcCommandError, tunRequestElevation: vi.fn() };
});

vi.mock("@/ipc", () => ipcMocks);

import { missingCorePayload, runWithElevation } from "./runtime-action";

const missingCoreError = new ipcMocks.IpcCommandError({
  kind: "missingCore",
  message: {
    candidates: [],
    coreType: "singBox",
    downloadUrl: "https://example.test/core",
    message: "sing-box is not installed",
    searchDir: "/cores",
  },
});

function elevationStatus(elevationGranted: boolean) {
  return { elevationGranted };
}

describe("runWithElevation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("returns the action result without asking for authorization", async () => {
    const action = vi.fn().mockResolvedValue("connected");

    await expect(runWithElevation(action)).resolves.toBe("connected");
    expect(action).toHaveBeenCalledTimes(1);
    expect(ipcMocks.tunRequestElevation).not.toHaveBeenCalled();
  });

  it("rethrows a failure that has nothing to do with elevation", async () => {
    const failure = new Error("core exited");
    const action = vi.fn().mockRejectedValue(failure);

    await expect(runWithElevation(action)).rejects.toBe(failure);
    expect(action).toHaveBeenCalledTimes(1);
    expect(ipcMocks.tunRequestElevation).not.toHaveBeenCalled();
  });

  it("rethrows a typed error of another kind without prompting", async () => {
    const action = vi.fn().mockRejectedValue(missingCoreError);

    await expect(runWithElevation(action)).rejects.toBe(missingCoreError);
    expect(ipcMocks.tunRequestElevation).not.toHaveBeenCalled();
  });

  it("requests authorization once and retries the action when it is granted", async () => {
    const failure = new ipcMocks.IpcCommandError({ kind: "sudo", message: "needs root" });
    const action = vi.fn().mockRejectedValueOnce(failure).mockResolvedValue("connected");
    ipcMocks.tunRequestElevation.mockResolvedValue(elevationStatus(true));

    await expect(runWithElevation(action)).resolves.toBe("connected");
    expect(ipcMocks.tunRequestElevation).toHaveBeenCalledTimes(1);
    expect(action).toHaveBeenCalledTimes(2);
  });

  it("treats a message mentioning authorization as an elevation failure", async () => {
    const failure = new ipcMocks.IpcCommandError(
      { kind: "tun", message: "tun failed" },
      "System Authorization was refused",
    );
    const action = vi.fn().mockRejectedValueOnce(failure).mockResolvedValue("connected");
    ipcMocks.tunRequestElevation.mockResolvedValue(elevationStatus(true));

    await expect(runWithElevation(action)).resolves.toBe("connected");
    expect(action).toHaveBeenCalledTimes(2);
  });

  it("rethrows the original failure when the authorization dialog is cancelled", async () => {
    const failure = new ipcMocks.IpcCommandError({ kind: "sudo", message: "needs root" });
    const action = vi.fn().mockRejectedValue(failure);
    ipcMocks.tunRequestElevation.mockResolvedValue(elevationStatus(false));

    await expect(runWithElevation(action)).rejects.toBe(failure);
    expect(ipcMocks.tunRequestElevation).toHaveBeenCalledTimes(1);
    // No retry: the user declined, so the action must not run a second time.
    expect(action).toHaveBeenCalledTimes(1);
  });

  it("propagates a retry that fails again", async () => {
    const first = new ipcMocks.IpcCommandError({ kind: "sudo", message: "needs root" });
    const second = new Error("still refused");
    const action = vi.fn().mockRejectedValueOnce(first).mockRejectedValueOnce(second);
    ipcMocks.tunRequestElevation.mockResolvedValue(elevationStatus(true));

    await expect(runWithElevation(action)).rejects.toBe(second);
    expect(action).toHaveBeenCalledTimes(2);
  });
});

describe("missingCorePayload", () => {
  it("extracts the core type and message from a typed missing-core failure", () => {
    expect(missingCorePayload(missingCoreError)).toEqual({
      coreType: "singBox",
      message: "sing-box is not installed",
    });
  });

  it("ignores plain errors and typed errors of other kinds", () => {
    expect(missingCorePayload(new Error("core missing"))).toBeNull();
    expect(missingCorePayload("core missing")).toBeNull();
    expect(
      missingCorePayload(new ipcMocks.IpcCommandError({ kind: "runtime", message: "boom" })),
    ).toBeNull();
  });
});
