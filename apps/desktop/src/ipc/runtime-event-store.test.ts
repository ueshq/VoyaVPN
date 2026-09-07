import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const ipcCommandMocks = vi.hoisted(() => ({
  speedtestStatus: vi.fn(),
}));

vi.mock("@/ipc/commands", () => ipcCommandMocks);

import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { ProxyConnectionsSnapshot, SpeedTestResult, StatisticsSnapshot } from "@/ipc/bindings";

const initialMonitorStatus = {
  message: null,
  running: false,
  stale: true,
  state: "stopped" as const,
};

const cachedConnections: ProxyConnectionsSnapshot = {
  connections: [],
  downloadTotal: 200,
  uploadTotal: 100,
};

describe("runtime event store", () => {
  beforeEach(() => {
    useRuntimeEventStore.setState({
      proxyConnections: null,
      proxyMonitorStatus: initialMonitorStatus,
      proxyTraffic: null,
      lastTransientEvent: null,
      logLines: [],
      serverStatsByProfileId: {},
      speedtestResultsByProfileId: {},
      speedtestRunning: false,
      statistics: null,
    });
    ipcCommandMocks.speedtestStatus.mockReset().mockResolvedValue({ running: false });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("stores proxy traffic websocket events", () => {
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyTraffic",
      payload: { down: 2048, up: 1024 },
    });

    expect(useRuntimeEventStore.getState().proxyTraffic).toEqual({ down: 2048, up: 1024 });
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("proxyTraffic");
  });

  it("hydrates speedtest running state from the backend status command", async () => {
    ipcCommandMocks.speedtestStatus.mockResolvedValue({ running: true });

    await useRuntimeEventStore.getState().refreshSpeedtestStatus();

    expect(ipcCommandMocks.speedtestStatus).toHaveBeenCalledTimes(1);
    expect(useRuntimeEventStore.getState().speedtestRunning).toBe(true);
  });

  it("sets speedtest running state through store actions", () => {
    useRuntimeEventStore.getState().setSpeedtestRunning(true);

    expect(useRuntimeEventStore.getState().speedtestRunning).toBe(true);

    useRuntimeEventStore.getState().setSpeedtestStatus({ running: false });

    expect(useRuntimeEventStore.getState().speedtestRunning).toBe(false);
  });

  it("stores speedtest result events without ending the running state", () => {
    const result: SpeedTestResult = {
      action: "latency",
      delay: 42,
      indexId: "profile-a",
      ipInfo: "US",
      message: "42",
      speed: null,
    };

    useRuntimeEventStore.getState().setSpeedtestRunning(true);
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "speedtestResult",
      payload: result,
    });

    expect(useRuntimeEventStore.getState().speedtestResultsByProfileId).toEqual({
      "profile-a": result,
    });
    expect(useRuntimeEventStore.getState().speedtestRunning).toBe(true);
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("speedtestResult");
  });

  it("sets proxy monitor lifecycle state through store actions", () => {
    useRuntimeEventStore.getState().setProxyMonitorRunning();

    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: null,
      running: true,
      stale: false,
      state: "running",
    });

    useRuntimeEventStore.getState().setProxyMonitorStarting("connecting");

    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: "connecting",
      running: false,
      stale: false,
      state: "starting",
    });

    useRuntimeEventStore.getState().setProxyMonitorStopped();

    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: null,
      running: false,
      stale: true,
      state: "stopped",
    });

    useRuntimeEventStore.getState().setProxyMonitorFailed("start failed");

    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: "start failed",
      running: false,
      stale: true,
      state: "failed",
    });
  });

  it("only clears stale state when fresh proxy traffic arrives", () => {
    useRuntimeEventStore.getState().setProxyMonitorFailed("stream failed");

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyTraffic",
      payload: { down: 2048, up: 1024 },
    });

    expect(useRuntimeEventStore.getState().proxyTraffic).toEqual({ down: 2048, up: 1024 });
    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: "stream failed",
      running: false,
      stale: false,
      state: "failed",
    });
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("proxyTraffic");
  });

  it("does not promote stopped monitor status when late proxy traffic arrives", () => {
    useRuntimeEventStore.getState().setProxyMonitorStopped("monitor stopped");

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyTraffic",
      payload: { down: 2048, up: 1024 },
    });

    expect(useRuntimeEventStore.getState().proxyTraffic).toEqual({ down: 2048, up: 1024 });
    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: "monitor stopped",
      running: false,
      stale: false,
      state: "stopped",
    });
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("proxyTraffic");
  });

  it("marks stopped monitor status stale while preserving proxy snapshots", () => {
    useRuntimeEventStore.getState().setProxyTraffic({ down: 2048, up: 1024 });
    useRuntimeEventStore.getState().setProxyConnections(cachedConnections);

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyMonitorStatus",
      payload: { state: "stopped", running: false, stale: true, message: null },
    });

    expect(useRuntimeEventStore.getState().proxyTraffic).toEqual({ down: 2048, up: 1024 });
    expect(useRuntimeEventStore.getState().proxyConnections).toEqual({
      connections: [],
      downloadTotal: 200,
      uploadTotal: 100,
    });
    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: null,
      running: false,
      stale: true,
      state: "stopped",
    });
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("proxyMonitorStatus");
  });

  it("marks failed monitor status stale with a message while preserving proxy snapshots", () => {
    useRuntimeEventStore.getState().setProxyTraffic({ down: 2048, up: 1024 });
    useRuntimeEventStore.getState().setProxyConnections(cachedConnections);

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyMonitorStatus",
      payload: { state: "failed", running: false, stale: true, message: "monitor failed" },
    });

    expect(useRuntimeEventStore.getState().proxyTraffic).toEqual({ down: 2048, up: 1024 });
    expect(useRuntimeEventStore.getState().proxyConnections).toEqual(cachedConnections);
    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: "monitor failed",
      running: false,
      stale: true,
      state: "failed",
    });
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("proxyMonitorStatus");
  });

  it("coalesces proxy connection websocket events into the next frame", async () => {
    vi.useFakeTimers();

    useRuntimeEventStore.getState().setProxyMonitorFailed("stream failed");

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyConnections",
      payload: makeConnectionsSnapshot("connection-1", "example.com:443", 200, 100),
    });
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyConnections",
      payload: makeConnectionsSnapshot("connection-2", "latest.example.com:443", 400, 300, ["Direct"]),
    });

    expect(useRuntimeEventStore.getState().proxyConnections).toBeNull();
    expect(useRuntimeEventStore.getState().proxyMonitorStatus.stale).toBe(true);

    await vi.advanceTimersByTimeAsync(20);

    const snapshot = useRuntimeEventStore.getState().proxyConnections;

    expect(snapshot?.connections[0]?.host).toBe("latest.example.com:443");
    expect(snapshot?.downloadTotal).toBe(400);
    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: "stream failed",
      running: false,
      stale: false,
      state: "failed",
    });
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("proxyConnections");
  });

  it("coalesces log lines into one frame and stamps each at receipt", async () => {
    vi.useFakeTimers();
    // Drive the receipt clock with a `Date.now` spy rather than `setSystemTime`:
    // moving the fake system clock after a frame is already scheduled stops that
    // frame from ever firing, which would make this assert a timer quirk instead
    // of the coalescing behaviour.
    const firstAt = Date.parse("2026-06-01T08:09:10.000Z");
    const secondAt = Date.parse("2026-06-01T08:10:30.000Z");
    const now = vi.spyOn(Date, "now");

    now.mockReturnValue(firstAt);
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "logLine",
      payload: { id: 1, level: "info", line: "core started" },
    });
    now.mockReturnValue(secondAt);
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "logLine",
      payload: { id: 2, level: "warn", line: "slow handshake" },
    });

    // One `set` per frame, not one per line: the core emits an event per stdout
    // line, which is hundreds per second at debug verbosity.
    expect(useRuntimeEventStore.getState().logLines).toEqual([]);

    await vi.advanceTimersByTimeAsync(20);

    // Each line keeps the moment it arrived, so lines buffered while the Logs
    // panel was unmounted do not all read as the panel-open time.
    expect(useRuntimeEventStore.getState().logLines).toEqual([
      { id: 1, level: "info", line: "core started", receivedAt: firstAt },
      { id: 2, level: "warn", line: "slow handshake", receivedAt: secondAt },
    ]);
    now.mockRestore();
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("logLine");
  });

  it("drops buffered log lines when the log is cleared before the frame runs", async () => {
    vi.useFakeTimers();

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "logLine",
      payload: { id: 1, level: "info", line: "core started" },
    });
    useRuntimeEventStore.getState().clearLogs();
    await vi.advanceTimersByTimeAsync(20);

    expect(useRuntimeEventStore.getState().logLines).toEqual([]);
  });

  it("stores a valid connections snapshot without copying every item", async () => {
    vi.useFakeTimers();
    const snapshot = makeConnectionsSnapshot("connection-1", "example.com:443", 200, 100);

    useRuntimeEventStore.getState().pushTransientEvent({ kind: "proxyConnections", payload: snapshot });
    await vi.advanceTimersByTimeAsync(20);

    // Same object: the payload comes from the specta-generated contract, so only
    // the envelope is checked instead of 14 fields per connection, once a second.
    expect(useRuntimeEventStore.getState().proxyConnections).toBe(snapshot);
  });

  it("rejects a connections payload whose envelope is malformed", async () => {
    vi.useFakeTimers();

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyConnections",
      payload: { connections: null, downloadTotal: 0, uploadTotal: 0 } as unknown as ProxyConnectionsSnapshot,
    });
    await vi.advanceTimersByTimeAsync(20);

    expect(useRuntimeEventStore.getState().proxyConnections).toBeNull();
  });

  it("rejects invalid statistics payloads before storing them", () => {
    const invalidStatistics = {
      activeProfileId: "profile-a",
      directDownloadBytesPerSecond: 0,
      directUploadBytesPerSecond: 0,
      downloadBytesPerSecond: 0,
      proxyDownloadBytesPerSecond: 0,
      proxyUploadBytesPerSecond: Number.NaN,
      serverStat: { indexId: "profile-a", totalUp: 1 },
      uploadBytesPerSecond: 0,
    } as StatisticsSnapshot;

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "statistics",
      payload: invalidStatistics,
    });

    expect(useRuntimeEventStore.getState().statistics).toBeNull();
    expect(useRuntimeEventStore.getState().serverStatsByProfileId).toEqual({});
    expect(useRuntimeEventStore.getState().lastTransientEvent).toBeNull();
  });

  it("does not let invalid proxy connection payloads replace a queued valid frame", async () => {
    vi.useFakeTimers();

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyConnections",
      payload: makeConnectionsSnapshot("connection-1", "valid.example.com:443", 200, 100),
    });
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyConnections",
      payload: {
        connections: [],
        downloadTotal: -1,
        uploadTotal: 0,
      } as ProxyConnectionsSnapshot,
    });

    await vi.advanceTimersByTimeAsync(20);

    expect(useRuntimeEventStore.getState().proxyConnections?.connections[0]?.host).toBe("valid.example.com:443");
    expect(useRuntimeEventStore.getState().proxyConnections?.downloadTotal).toBe(200);
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("proxyConnections");
  });
});

function makeConnectionsSnapshot(
  id: string,
  host: string,
  downloadTotal: number,
  uploadTotal: number,
  chains = ["Proxy"],
): ProxyConnectionsSnapshot {
  return {
    connections: [
      {
        chains,
        connectionType: "HTTP",
        destination: "93.184.216.34:443",
        download: downloadTotal,
        host,
        id,
        network: "tcp",
        process: "browser",
        processPath: "/usr/bin/browser",
        rule: "MATCH",
        rulePayload: null,
        source: "127.0.0.1:53000",
        start: "2026-06-01T00:00:00Z",
        upload: uploadTotal,
      },
    ],
    downloadTotal,
    uploadTotal,
  };
}
