import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { installFakeCommands } from "@voya/features/test/backend";

type TestMonitorStatus = {
  message: string | null;
  running: boolean;
  stale: boolean;
  state: string;
};

// The snapshot lives outside `state` so the store actions can rewrite it without
// `state` referencing itself in its own initializer.
const storeMock = vi.hoisted(() => {
  const stopped: TestMonitorStatus = { message: null, running: false, stale: true, state: "stopped" };
  const snapshot = { value: stopped };
  const state = {
    get proxyMonitorStatus() {
      return snapshot.value;
    },
    setProxyMonitorFailed: vi.fn((message: string | null = null) => {
      snapshot.value = { message, running: false, stale: true, state: "failed" };
    }),
    setProxyMonitorStarting: vi.fn(() => {
      snapshot.value = { ...snapshot.value, running: false, state: "starting" };
    }),
    setProxyMonitorStatus: vi.fn((status: TestMonitorStatus) => {
      snapshot.value = status;
    }),
  };

  return {
    reset() {
      snapshot.value = stopped;
      state.setProxyMonitorFailed.mockClear();
      state.setProxyMonitorStarting.mockClear();
      state.setProxyMonitorStatus.mockClear();
    },
    setStatus(next: TestMonitorStatus) {
      snapshot.value = next;
    },
    state,
  };
});

const ipcMocks = installFakeCommands({
  proxyStartMonitor: vi.fn(),
  proxyStopMonitor: vi.fn(),
});

vi.mock("@voya/client/runtime-event-store", () => ({ useRuntimeEventStore: { getState: () => storeMock.state } }));

import { createProxyMonitorController } from "./proxy-monitor-controller";

const RUNNING: TestMonitorStatus = { message: null, running: true, stale: false, state: "running" };
const STOPPED: TestMonitorStatus = { message: null, running: false, stale: true, state: "stopped" };

describe("proxy monitor controller", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    storeMock.reset();
    ipcMocks.proxyStartMonitor.mockReset().mockResolvedValue(RUNNING);
    ipcMocks.proxyStopMonitor.mockReset().mockResolvedValue(STOPPED);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function makeController(onError = vi.fn()) {
    return { controller: createProxyMonitorController({ onError }), onError };
  }

  it("debounces the start so a tab passed through opens no socket", async () => {
    const { controller } = makeController();

    controller.setWanted(true);
    await vi.advanceTimersByTimeAsync(99);
    expect(ipcMocks.proxyStartMonitor).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(ipcMocks.proxyStartMonitor).toHaveBeenCalledTimes(1);
    // The "starting" badge is set before the command, never after it.
    expect(storeMock.state.setProxyMonitorStarting.mock.invocationCallOrder[0]!).toBeLessThan(
      ipcMocks.proxyStartMonitor.mock.invocationCallOrder[0]!,
    );
    expect(storeMock.state.proxyMonitorStatus).toEqual(RUNNING);
  });

  it("leaves the monitor alone while switching between proxy surfaces", async () => {
    const { controller } = makeController();

    controller.setWanted(true);
    await vi.advanceTimersByTimeAsync(50);
    // A second proxy surface takes over before the first start even fired.
    controller.setWanted(true);
    await vi.advanceTimersByTimeAsync(100);
    expect(ipcMocks.proxyStartMonitor).toHaveBeenCalledTimes(1);

    controller.setWanted(false);
    controller.setWanted(true);
    await vi.advanceTimersByTimeAsync(5_000);

    expect(ipcMocks.proxyStartMonitor).toHaveBeenCalledTimes(1);
    expect(ipcMocks.proxyStopMonitor).not.toHaveBeenCalled();
  });

  it("stops only after the grace period once no surface wants it", async () => {
    const { controller } = makeController();

    controller.setWanted(true);
    await vi.advanceTimersByTimeAsync(100);
    controller.setWanted(false);

    await vi.advanceTimersByTimeAsync(1_999);
    expect(ipcMocks.proxyStopMonitor).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(ipcMocks.proxyStopMonitor).toHaveBeenCalledTimes(1);
    expect(storeMock.state.proxyMonitorStatus).toEqual(STOPPED);
  });

  it("reports a rejected start and does not leave the monitor marked running", async () => {
    const { controller, onError } = makeController();
    const failure = new Error("start unavailable");
    ipcMocks.proxyStartMonitor.mockRejectedValueOnce(failure);

    controller.setWanted(true);
    await vi.advanceTimersByTimeAsync(100);

    expect(onError).toHaveBeenCalledWith(failure, "start");
    expect(storeMock.state.setProxyMonitorStatus).not.toHaveBeenCalled();
    expect(storeMock.state.proxyMonitorStatus.running).toBe(false);
  });

  it("reports a rejected stop", async () => {
    const { controller, onError } = makeController();
    const failure = new Error("stop unavailable");
    ipcMocks.proxyStopMonitor.mockRejectedValueOnce(failure);

    controller.setWanted(true);
    await vi.advanceTimersByTimeAsync(100);
    controller.setWanted(false);
    await vi.advanceTimersByTimeAsync(2_000);

    expect(onError).toHaveBeenCalledWith(failure, "stop");
  });

  it("stops a start that completed after the surface went away", async () => {
    const { controller } = makeController();
    let resolveStart: ((status: TestMonitorStatus) => void) | undefined;
    ipcMocks.proxyStartMonitor.mockReturnValueOnce(
      new Promise<TestMonitorStatus>((resolve) => {
        resolveStart = resolve;
      }),
    );

    controller.setWanted(true);
    await vi.advanceTimersByTimeAsync(100);
    // The start is in flight and the store still says "not running", so the
    // in-flight flag is what has to keep the stop scheduled.
    controller.setWanted(false);

    resolveStart?.(RUNNING);
    await vi.advanceTimersByTimeAsync(2_100);

    expect(ipcMocks.proxyStopMonitor).toHaveBeenCalledTimes(1);
  });

  it("restarts when a surface reappears while the stop is in flight", async () => {
    const { controller } = makeController();
    let resolveStop: ((status: TestMonitorStatus) => void) | undefined;
    ipcMocks.proxyStopMonitor.mockReturnValueOnce(
      new Promise<TestMonitorStatus>((resolve) => {
        resolveStop = resolve;
      }),
    );

    controller.setWanted(true);
    await vi.advanceTimersByTimeAsync(100);
    controller.setWanted(false);
    await vi.advanceTimersByTimeAsync(2_000);
    expect(ipcMocks.proxyStopMonitor).toHaveBeenCalledTimes(1);

    controller.setWanted(true);
    resolveStop?.(STOPPED);
    await vi.advanceTimersByTimeAsync(200);

    expect(ipcMocks.proxyStartMonitor).toHaveBeenCalledTimes(2);
  });

  it("treats a backend-pushed status as the truth about running", async () => {
    const { controller } = makeController();

    controller.setWanted(true);
    await vi.advanceTimersByTimeAsync(100);
    expect(storeMock.state.proxyMonitorStatus.running).toBe(true);

    // The websocket loop failed on its own and pushed a status; the controller
    // keeps no private copy that could disagree with it.
    storeMock.setStatus({ message: "socket closed", running: false, stale: true, state: "failed" });
    controller.setWanted(false);
    await vi.advanceTimersByTimeAsync(5_000);

    expect(ipcMocks.proxyStopMonitor).not.toHaveBeenCalled();
  });

  it("stops immediately on dispose and then ignores further requests", async () => {
    const { controller } = makeController();

    controller.setWanted(true);
    await vi.advanceTimersByTimeAsync(100);
    controller.dispose();

    expect(ipcMocks.proxyStopMonitor).toHaveBeenCalledTimes(1);

    controller.setWanted(true);
    await vi.advanceTimersByTimeAsync(5_000);
    expect(ipcMocks.proxyStartMonitor).toHaveBeenCalledTimes(1);
  });
});
