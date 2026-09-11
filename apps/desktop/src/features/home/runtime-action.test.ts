import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AppError, AppErrorKind, RuntimeStatusResponse } from "@/ipc/bindings";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { beginRuntimeRead } from "@/ipc/runtime-state-version";

const ipcMocks = vi.hoisted(() => {
  // A faithful stand-in for the real error: `runWithElevation` and
  // `missingCorePayload` both branch on `appError.kind`, so a bare
  // `class extends Error {}` would make every branch below unreachable. The
  // real class also takes its `Error.message` straight from `appError.message`,
  // which is what makes "the message no longer decides anything" testable.
  class MockIpcCommandError extends Error {
    readonly appError: AppError;

    constructor(appError: AppError) {
      super(appError.message);
      this.appError = appError;
      this.name = "IpcCommandError";
    }
  }

  return {
    IpcCommandError: MockIpcCommandError,
    tunRequestElevation: vi.fn(),
    connectActiveProfile: vi.fn(),
    disconnectCore: vi.fn(),
    restartCore: vi.fn(),
  };
});

vi.mock("@/ipc/commands", () => ipcMocks);

import { executeRuntimeAction, missingCorePayload, runWithElevation } from "./runtime-action";

function appError(kind: AppErrorKind, message = "ipc failed"): AppError {
  return { kind, message, subsystem: "runtime" };
}

const missingCoreError = new ipcMocks.IpcCommandError(
  appError(
    {
      candidates: [],
      coreType: "singBox",
      downloadUrl: "https://example.test/core",
      searchDir: "/cores",
      type: "missingCore",
    },
    "sing-box is not installed",
  ),
);

function elevationStatus(elevationGranted: boolean) {
  return { elevationGranted };
}

describe("runtime command responses", () => {
  const connected: RuntimeStatusResponse = {
    activeProfileId: "node", activeTunBackend: null, mainPid: 42, prePid: null,
    runningCoreType: "singBox", state: "connected", connectedDurationMs: 0,
  };

  beforeEach(() => {
    vi.resetAllMocks();
    useRuntimeEventStore.setState({ coreState: null });
  });

  it.each([
    ["connect", "connectActiveProfile"],
    ["disconnect", "disconnectCore"],
    ["restart", "restartCore"],
  ] as const)("runs %s and stores its current response", async (action, command) => {
    ipcMocks[command].mockResolvedValueOnce(connected);
    await expect(executeRuntimeAction(action)).resolves.toBe(connected);
    expect(ipcMocks[command]).toHaveBeenCalledExactlyOnceWith();
    expect(useRuntimeEventStore.getState().coreState).toEqual(connected);
  });

  it.each(["event", "read"] as const)("keeps a newer %s when an older command completes", async (source) => {
    let resolve!: (status: RuntimeStatusResponse) => void;
    ipcMocks.connectActiveProfile.mockReturnValueOnce(new Promise<RuntimeStatusResponse>((done) => { resolve = done; }));
    const pending = executeRuntimeAction("connect");
    const newer = { ...connected, mainPid: 99 };
    if (source === "event") {
      useRuntimeEventStore.getState().pushTransientEvent({ kind: "coreState", payload: newer });
    } else {
      beginRuntimeRead("coreState");
      useRuntimeEventStore.getState().setCoreState(newer);
    }
    resolve(connected);
    await expect(pending).resolves.toBe(connected);
    expect(useRuntimeEventStore.getState().coreState).toEqual(newer);
  });
});

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
    const failure = new ipcMocks.IpcCommandError(
      appError(
        { type: "elevationRequired" },
        "system authorization is required before enabling TUN on Unix",
      ),
    );
    const action = vi.fn().mockRejectedValueOnce(failure).mockResolvedValue("connected");
    ipcMocks.tunRequestElevation.mockResolvedValue(elevationStatus(true));

    await expect(runWithElevation(action)).resolves.toBe("connected");
    expect(ipcMocks.tunRequestElevation).toHaveBeenCalledTimes(1);
    expect(action).toHaveBeenCalledTimes(2);
  });

  // The falsification that matters: the retry used to fire on any message
  // containing "authorization". Rewording — or translating — the backend text
  // must not change what happens.
  it("retries on the typed kind alone, whatever the message says", async () => {
    const failure = new ipcMocks.IpcCommandError(
      appError({ type: "elevationRequired" }, "系统需要一次性授权"),
    );
    const action = vi.fn().mockRejectedValueOnce(failure).mockResolvedValue("connected");
    ipcMocks.tunRequestElevation.mockResolvedValue(elevationStatus(true));

    await expect(runWithElevation(action)).resolves.toBe("connected");
    expect(ipcMocks.tunRequestElevation).toHaveBeenCalledTimes(1);
    expect(action).toHaveBeenCalledTimes(2);
  });

  // The other half of the same falsification. `native authorization was
  // cancelled` is what the elevation dialog reports when the user declines, and
  // the old substring match read it as "ask again", re-opening the dialog and
  // re-running the connect.
  it("does not prompt for a failure that merely mentions authorization", async () => {
    const failure = new ipcMocks.IpcCommandError(
      appError({ type: "internal" }, "native authorization was cancelled"),
    );
    const action = vi.fn().mockRejectedValue(failure);

    await expect(runWithElevation(action)).rejects.toBe(failure);
    expect(ipcMocks.tunRequestElevation).not.toHaveBeenCalled();
    expect(action).toHaveBeenCalledTimes(1);
  });

  it("rethrows the original failure when the authorization dialog is cancelled", async () => {
    const failure = new ipcMocks.IpcCommandError(appError({ type: "elevationRequired" }));
    const action = vi.fn().mockRejectedValue(failure);
    ipcMocks.tunRequestElevation.mockResolvedValue(elevationStatus(false));

    await expect(runWithElevation(action)).rejects.toBe(failure);
    expect(ipcMocks.tunRequestElevation).toHaveBeenCalledTimes(1);
    // No retry: the user declined, so the action must not run a second time.
    expect(action).toHaveBeenCalledTimes(1);
  });

  it("propagates a retry that fails again", async () => {
    const first = new ipcMocks.IpcCommandError(appError({ type: "elevationRequired" }));
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
      missingCorePayload(new ipcMocks.IpcCommandError(appError({ type: "internal" }, "boom"))),
    ).toBeNull();
  });
});
