import { beforeEach, describe, expect, it, vi } from "vitest";

import type { TranslationFunction } from "@voya/i18n";
import type {
  AppError,
  AppErrorKind,
  RuntimeStatusResponse,
  VoyaCommands,
} from "@voya/contracts";
import { useRuntimeEventStore } from "./runtime-event-store";
import { beginRuntimeRead } from "./runtime-state-version";
import { useRuntimeActionStore } from "./runtime-action-store";
import { useToastStore } from "./toast-store";
import { setElevationHandler } from "./platform";
import { setVoyaCommands } from "./transport";

const runtimeStatus = vi.hoisted(() => ({ refreshRuntimeStatusAndReport: vi.fn() }));
vi.mock("./runtime-status", () => runtimeStatus);

// The kind check and the error class stay real: `runWithElevation` and
// `missingCorePayload` both branch on `appError.kind`, and the real class takes
// its `Error.message` straight from `appError.message`, which is what makes
// "the message no longer decides anything" testable.
const ipcMocks = {
  connectActiveProfile: vi.fn(),
  disconnectCore: vi.fn(),
  restartCore: vi.fn(),
};
setVoyaCommands(ipcMocks as unknown as VoyaCommands);

// The platform's authorization prompt, injected the way an app registers it.
const requestElevation = vi.fn<() => Promise<boolean>>();
setElevationHandler(requestElevation);

import { IpcCommandError } from "./errors";
import {
  activateSelection,
  executeRuntimeAction,
  missingCorePayload,
  runRuntimeAction,
  runWithElevation,
} from "./runtime-action";

// Keys stand in for text, so assertions do not depend on a locale.
const t = ((key: string) => key) as unknown as TranslationFunction;

function appError(kind: AppErrorKind, message = "ipc failed"): AppError {
  return { kind, message, subsystem: "runtime" };
}

const missingCoreError = new IpcCommandError(
  appError(
    {
      candidates: [],
      downloadUrl: "https://example.test/core",
      searchDir: "/cores",
      type: "missingCore",
    },
    "sing-box is not installed",
  ),
);

function coreStatus(state: RuntimeStatusResponse["state"]): RuntimeStatusResponse {
  const connected = state === "connected";
  return {
    activeProfileId: connected ? "node" : null,
    activeTunBackend: null,
    connectedDurationMs: null,
    mainPid: connected ? 42 : null,
    prePid: null,
    state,
  };
}

function resetStores() {
  useRuntimeEventStore.setState({ coreState: null });
  useRuntimeActionStore.setState({ lastError: null, modePending: false, pendingAction: null, switchingId: null });
  useRuntimeActionStore.setState({ missingCore: null });
  useToastStore.setState({ toasts: [] });
}

describe("runtime command responses", () => {
  const connected: RuntimeStatusResponse = {
    activeProfileId: "node", activeTunBackend: null, mainPid: 42, prePid: null,
    state: "connected", connectedDurationMs: 0,
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
    expect(requestElevation).not.toHaveBeenCalled();
  });

  it("rethrows a failure that has nothing to do with elevation", async () => {
    const failure = new Error("core exited");
    const action = vi.fn().mockRejectedValue(failure);

    await expect(runWithElevation(action)).rejects.toBe(failure);
    expect(action).toHaveBeenCalledTimes(1);
    expect(requestElevation).not.toHaveBeenCalled();
  });

  it("rethrows a typed error of another kind without prompting", async () => {
    const action = vi.fn().mockRejectedValue(missingCoreError);

    await expect(runWithElevation(action)).rejects.toBe(missingCoreError);
    expect(requestElevation).not.toHaveBeenCalled();
  });

  it("requests authorization once and retries the action when it is granted", async () => {
    const failure = new IpcCommandError(
      appError(
        { type: "elevationRequired" },
        "system authorization is required before enabling TUN on Unix",
      ),
    );
    const action = vi.fn().mockRejectedValueOnce(failure).mockResolvedValue("connected");
    requestElevation.mockResolvedValue(true);

    await expect(runWithElevation(action)).resolves.toBe("connected");
    expect(requestElevation).toHaveBeenCalledTimes(1);
    expect(action).toHaveBeenCalledTimes(2);
  });

  // The falsification that matters: the retry used to fire on any message
  // containing "authorization". Rewording — or translating — the backend text
  // must not change what happens.
  it("retries on the typed kind alone, whatever the message says", async () => {
    const failure = new IpcCommandError(
      appError({ type: "elevationRequired" }, "系统需要一次性授权"),
    );
    const action = vi.fn().mockRejectedValueOnce(failure).mockResolvedValue("connected");
    requestElevation.mockResolvedValue(true);

    await expect(runWithElevation(action)).resolves.toBe("connected");
    expect(requestElevation).toHaveBeenCalledTimes(1);
    expect(action).toHaveBeenCalledTimes(2);
  });

  // The other half of the same falsification. `native authorization was
  // cancelled` is what the elevation dialog reports when the user declines, and
  // the old substring match read it as "ask again", re-opening the dialog and
  // re-running the connect.
  it("does not prompt for a failure that merely mentions authorization", async () => {
    const failure = new IpcCommandError(
      appError({ type: "internal" }, "native authorization was cancelled"),
    );
    const action = vi.fn().mockRejectedValue(failure);

    await expect(runWithElevation(action)).rejects.toBe(failure);
    expect(requestElevation).not.toHaveBeenCalled();
    expect(action).toHaveBeenCalledTimes(1);
  });

  it("rethrows the original failure when the authorization dialog is cancelled", async () => {
    const failure = new IpcCommandError(appError({ type: "elevationRequired" }));
    const action = vi.fn().mockRejectedValue(failure);
    requestElevation.mockResolvedValue(false);

    await expect(runWithElevation(action)).rejects.toBe(failure);
    expect(requestElevation).toHaveBeenCalledTimes(1);
    // No retry: the user declined, so the action must not run a second time.
    expect(action).toHaveBeenCalledTimes(1);
  });

  it("propagates a retry that fails again", async () => {
    const first = new IpcCommandError(appError({ type: "elevationRequired" }));
    const second = new Error("still refused");
    const action = vi.fn().mockRejectedValueOnce(first).mockRejectedValueOnce(second);
    requestElevation.mockResolvedValue(true);

    await expect(runWithElevation(action)).rejects.toBe(second);
    expect(action).toHaveBeenCalledTimes(2);
  });
});

describe("runRuntimeAction", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    resetStores();
  });

  it("guards the action while it runs and reads the status back afterwards", async () => {
    ipcMocks.connectActiveProfile.mockResolvedValueOnce(coreStatus("connected"));

    const running = runRuntimeAction("connect", t);
    expect(useRuntimeActionStore.getState().pendingAction).toBe("connect");
    await running;

    expect(runtimeStatus.refreshRuntimeStatusAndReport).toHaveBeenCalledWith(t);
    expect(useRuntimeActionStore.getState().pendingAction).toBeNull();
  });

  it("does nothing while another runtime action or a transition is under way", async () => {
    useRuntimeActionStore.setState({ switchingId: "node" });
    await runRuntimeAction("connect", t);
    useRuntimeActionStore.setState({ switchingId: null });
    useRuntimeEventStore.setState({ coreState: coreStatus("connecting") });
    await runRuntimeAction("connect", t);

    expect(ipcMocks.connectActiveProfile).not.toHaveBeenCalled();
    expect(runtimeStatus.refreshRuntimeStatusAndReport).not.toHaveBeenCalled();
  });

  it("keeps a failure next to Home's button, and toasts it anywhere else", async () => {
    ipcMocks.connectActiveProfile.mockRejectedValueOnce(new Error("core exited"));
    await runRuntimeAction("connect", t, { inline: true });
    expect(useRuntimeActionStore.getState()).toMatchObject({
      lastError: { action: "connect", message: "core exited" },
      pendingAction: null,
    });

    ipcMocks.connectActiveProfile.mockRejectedValueOnce(new Error("still offline"));
    await runRuntimeAction("connect", t);
    // Starting again drops the earlier inline failure.
    expect(useRuntimeActionStore.getState().lastError).toBeNull();
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "still offline",
      severity: "error",
      title: "actions.connect",
    });
  });

  it("opens the missing-core dialog instead of reporting the failure", async () => {
    ipcMocks.connectActiveProfile.mockRejectedValueOnce(missingCoreError);

    await runRuntimeAction("connect", t);

    expect(useRuntimeActionStore.getState().missingCore).toEqual({
      message: "sing-box is not installed",
    });
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });

  it("explains a declined authorization instead of the raw failure", async () => {
    ipcMocks.connectActiveProfile.mockRejectedValue(
      new IpcCommandError(appError({ type: "elevationRequired" }, "sudo helper refused")),
    );
    requestElevation.mockResolvedValue(false);

    await runRuntimeAction("connect", t, { inline: true });

    expect(useRuntimeActionStore.getState().lastError).toEqual({
      action: "connect",
      message: "home.authorizationDeclined",
    });
  });
});

describe("activateSelection", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    resetStores();
  });

  it("saves the selection before connecting an idle core", async () => {
    const select = vi.fn().mockResolvedValue(undefined);
    ipcMocks.connectActiveProfile.mockResolvedValueOnce(coreStatus("connected"));

    const switching = activateSelection("node", t, select);
    expect(useRuntimeActionStore.getState().switchingId).toBe("node");

    await expect(switching).resolves.toBe(true);
    expect(select.mock.invocationCallOrder[0]!).toBeLessThan(
      ipcMocks.connectActiveProfile.mock.invocationCallOrder[0]!,
    );
    expect(runtimeStatus.refreshRuntimeStatusAndReport).toHaveBeenCalledWith(t);
    expect(useRuntimeActionStore.getState().switchingId).toBeNull();
  });

  it("restarts a running core with the new selection", async () => {
    useRuntimeEventStore.setState({ coreState: coreStatus("connected") });
    ipcMocks.restartCore.mockResolvedValueOnce(coreStatus("connected"));

    await expect(activateSelection("group:work", t, vi.fn().mockResolvedValue(undefined))).resolves.toBe(true);

    expect(ipcMocks.restartCore).toHaveBeenCalledOnce();
    expect(ipcMocks.connectActiveProfile).not.toHaveBeenCalled();
  });

  it("changes nothing while the core cleans up or another action runs", async () => {
    const select = vi.fn();
    useRuntimeEventStore.setState({ coreState: coreStatus("cleanupPending") });
    await expect(activateSelection("node", t, select)).resolves.toBe(false);

    useRuntimeEventStore.setState({ coreState: null });
    useRuntimeActionStore.setState({ modePending: true });
    await expect(activateSelection("node", t, select)).resolves.toBe(false);

    expect(select).not.toHaveBeenCalled();
    expect(runtimeStatus.refreshRuntimeStatusAndReport).not.toHaveBeenCalled();
  });

  it("reports a selection that could not be saved and never starts the core", async () => {
    const select = vi.fn().mockRejectedValue(new Error("node not found"));

    await expect(activateSelection("node", t, select)).resolves.toBe(false);

    expect(ipcMocks.connectActiveProfile).not.toHaveBeenCalled();
    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "node not found",
      title: "actions.connect",
    });
    expect(useRuntimeActionStore.getState().switchingId).toBeNull();
  });
});

describe("missingCorePayload", () => {
  it("extracts the core type and message from a typed missing-core failure", () => {
    expect(missingCorePayload(missingCoreError)).toEqual({
      message: "sing-box is not installed",
    });
  });

  it("ignores plain errors and typed errors of other kinds", () => {
    expect(missingCorePayload(new Error("core missing"))).toBeNull();
    expect(missingCorePayload("core missing")).toBeNull();
    expect(
      missingCorePayload(new IpcCommandError(appError({ type: "internal" }, "boom"))),
    ).toBeNull();
  });
});
