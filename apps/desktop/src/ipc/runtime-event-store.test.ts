import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const ipcCommandMocks = vi.hoisted(() => ({
  speedtestStatus: vi.fn(),
}));

vi.mock("@/ipc/commands", () => ipcCommandMocks);

import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { ProxyConnectionsSnapshot, SpeedtestResult, StatisticsSnapshot } from "@/ipc/bindings";

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

  it("hydrates speedtest running state from the backend status command", async () => {
    ipcCommandMocks.speedtestStatus.mockResolvedValue({ running: true });

    await useRuntimeEventStore.getState().refreshSpeedtestStatus();

    expect(ipcCommandMocks.speedtestStatus).toHaveBeenCalledTimes(1);
    expect(useRuntimeEventStore.getState().speedtestRunning).toBe(true);
  });

  it("sets speedtest running state through store actions", () => {
    useRuntimeEventStore.getState().setSpeedtestRunning(true);

    expect(useRuntimeEventStore.getState().speedtestRunning).toBe(true);

    useRuntimeEventStore.getState().setSpeedtestRunning(false);

    expect(useRuntimeEventStore.getState().speedtestRunning).toBe(false);
  });

  it("stores speedtest result events without ending the running state", async () => {
    vi.useFakeTimers();
    const result: SpeedtestResult = {
      delay: 42,
      indexId: "profile-a",
      detail: null,
      ipInfo: "US",
      countryCode: null,
      outcome: "completed",
    };

    useRuntimeEventStore.getState().setSpeedtestRunning(true);
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "speedtestResults",
      payload: [result],
    });

    // One `set` per frame, not one per node: a run reports a result each time
    // a probe completes, and each stored result rebuilds the node table.
    expect(useRuntimeEventStore.getState().speedtestResultsByProfileId).toEqual({});

    await vi.advanceTimersByTimeAsync(20);

    expect(useRuntimeEventStore.getState().speedtestResultsByProfileId).toEqual({
      "profile-a": result,
    });
    expect(useRuntimeEventStore.getState().speedtestRunning).toBe(true);

    useRuntimeEventStore.getState().clearSpeedtestResults();
    expect(useRuntimeEventStore.getState().speedtestResultsByProfileId).toEqual({});
    expect(useRuntimeEventStore.getState().speedtestRunning).toBe(true);
  });

  it("coalesces a burst of speedtest results into one frame with the latest per node", async () => {
    vi.useFakeTimers();

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "speedtestResults",
      payload: [speedtestResult("profile-a", "testing")],
    });
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "speedtestResults",
      payload: [speedtestResult("profile-b", "completed", 42)],
    });
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "speedtestResults",
      payload: [speedtestResult("profile-a", "completed", 12)],
    });

    expect(useRuntimeEventStore.getState().speedtestResultsByProfileId).toEqual({});

    await vi.advanceTimersByTimeAsync(20);

    expect(useRuntimeEventStore.getState().speedtestResultsByProfileId).toEqual({
      "profile-a": speedtestResult("profile-a", "completed", 12),
      "profile-b": speedtestResult("profile-b", "completed", 42),
    });

    // Results arriving after a flush schedule a new frame and merge on top of
    // what is already stored.
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "speedtestResults",
      payload: [speedtestResult("profile-c", "timedOut")],
    });
    await vi.advanceTimersByTimeAsync(20);

    expect(Object.keys(useRuntimeEventStore.getState().speedtestResultsByProfileId)).toEqual([
      "profile-a",
      "profile-b",
      "profile-c",
    ]);
  });

  it("stores every result of a batched speedtest event, the later one per node winning", async () => {
    vi.useFakeTimers();

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "speedtestResults",
      payload: [
        speedtestResult("profile-a", "testing"),
        speedtestResult("profile-b", "testing"),
        speedtestResult("profile-a", "cancelled"),
      ],
    });
    await vi.advanceTimersByTimeAsync(20);

    expect(useRuntimeEventStore.getState().speedtestResultsByProfileId).toEqual({
      "profile-a": speedtestResult("profile-a", "cancelled"),
      "profile-b": speedtestResult("profile-b", "testing"),
    });
  });

  it("drops buffered speedtest results when the overlay is cleared before the frame runs", async () => {
    vi.useFakeTimers();

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "speedtestResults",
      payload: [speedtestResult("profile-a", "completed", 42)],
    });
    useRuntimeEventStore.getState().clearSpeedtestResults();
    await vi.advanceTimersByTimeAsync(20);

    expect(useRuntimeEventStore.getState().speedtestResultsByProfileId).toEqual({});
  });

  it("sets proxy monitor lifecycle state through store actions", () => {
    useRuntimeEventStore.getState().setProxyMonitorStatus({
      message: null,
      running: true,
      stale: false,
      state: "running",
    });

    useRuntimeEventStore.getState().setProxyMonitorStarting("connecting");

    // A restart keeps showing the data it already has as fresh.
    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: "connecting",
      running: false,
      stale: false,
      state: "starting",
    });

    useRuntimeEventStore.getState().setProxyMonitorFailed("start failed");

    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: "start failed",
      running: false,
      stale: true,
      state: "failed",
    });
  });

  it("does not promote stopped monitor status when late proxy connections arrive", async () => {
    vi.useFakeTimers();
    useRuntimeEventStore.getState().setProxyMonitorStatus({
      message: "monitor stopped",
      running: false,
      stale: true,
      state: "stopped",
    });

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyConnections",
      payload: makeConnectionsSnapshot("connection-1", "example.com:443", 200, 100),
    });
    await vi.advanceTimersByTimeAsync(20);

    // Data arriving clears staleness only. The monitor stays stopped until a
    // lifecycle event says otherwise.
    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: "monitor stopped",
      running: false,
      stale: false,
      state: "stopped",
    });
    expect(useRuntimeEventStore.getState().proxyConnections?.connections[0]?.id).toBe("connection-1");
  });

  it("marks stopped monitor status stale while preserving proxy snapshots", () => {
    useRuntimeEventStore.getState().setProxyConnections(cachedConnections);

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyMonitorStatus",
      payload: { state: "stopped", running: false, stale: true, message: null },
    });

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
  });

  it("marks failed monitor status stale with a message while preserving proxy snapshots", () => {
    useRuntimeEventStore.getState().setProxyConnections(cachedConnections);

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyMonitorStatus",
      payload: { state: "failed", running: false, stale: true, message: "monitor failed" },
    });

    expect(useRuntimeEventStore.getState().proxyConnections).toEqual(cachedConnections);
    expect(useRuntimeEventStore.getState().proxyMonitorStatus).toEqual({
      message: "monitor failed",
      running: false,
      stale: true,
      state: "failed",
    });
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
  });

  it("coalesces log batches into one frame and stamps each at receipt", async () => {
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
      kind: "logLines",
      payload: [
        { body: { line: "core started", source: "core" }, id: 1, level: "info" },
        { body: { line: "inbound/mixed started", source: "core" }, id: 2, level: "info" },
      ],
    });
    now.mockReturnValue(secondAt);
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "logLines",
      payload: [
        {
          body: { code: { code: "connected" }, detail: null, source: "app" },
          id: 3,
          level: "warn",
        },
      ],
    });

    // One `set` per frame, not one per batch: two batches can land between
    // frames.
    expect(useRuntimeEventStore.getState().logLines).toEqual([]);

    await vi.advanceTimersByTimeAsync(20);

    // Each line keeps the moment it arrived, so lines buffered while the Logs
    // panel was unmounted do not all read as the panel-open time.
    expect(useRuntimeEventStore.getState().logLines).toEqual([
      { body: { line: "core started", source: "core" }, id: 1, level: "info", receivedAt: firstAt },
      {
        body: { line: "inbound/mixed started", source: "core" },
        id: 2,
        level: "info",
        receivedAt: firstAt,
      },
      {
        body: { code: { code: "connected" }, detail: null, source: "app" },
        id: 3,
        level: "warn",
        receivedAt: secondAt,
      },
    ]);
    now.mockRestore();
  });

  it("drops buffered log lines when the log is cleared before the frame runs", async () => {
    vi.useFakeTimers();

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "logLines",
      payload: [{ body: { line: "core started", source: "core" }, id: 1, level: "info" }],
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

  it("stores a valid statistics payload with only its known fields", () => {
    const statistics: StatisticsSnapshot = {
      activeProfileId: "profile-a",
      directDownloadBytesPerSecond: 1,
      directUploadBytesPerSecond: 2,
      downloadBytesPerSecond: 3,
      proxyDownloadBytesPerSecond: null,
      proxyUploadBytesPerSecond: 4,
      serverStat: {
        dateNow: 5,
        indexId: "profile-a",
        todayDown: 6,
        todayUp: 7,
        totalDown: null,
        totalUp: 8,
      },
      uploadBytesPerSecond: 0,
    };

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "statistics",
      payload: { ...statistics, extra: "dropped" } as StatisticsSnapshot,
    });

    expect(useRuntimeEventStore.getState().statistics).toEqual(statistics);
    expect(useRuntimeEventStore.getState().serverStatsByProfileId).toEqual({
      "profile-a": statistics.serverStat,
    });
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
  });
});

function speedtestResult(
  indexId: string,
  outcome: SpeedtestResult["outcome"],
  delay: number | null = null,
): SpeedtestResult {
  return {
    countryCode: null,
    delay,
    detail: null,
    indexId,
    ipInfo: null,
    outcome,
  };
}

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
