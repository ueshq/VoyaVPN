import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ProxyConnectionsSnapshot,
  RuntimeStatusResponse,
  SpeedtestResult,
  StatisticsSnapshot,
  VoyaCommands,
} from "@voya/contracts";

import { useRuntimeEventStore } from "./runtime-event-store";
import { setVoyaCommands } from "./transport";

// The store reaches the backend through the registered command surface, so the
// test registers a stub platform rather than mocking a transport module.
const ipcCommandMocks = {
  speedtestStatus: vi.fn(),
};

setVoyaCommands(ipcCommandMocks as unknown as VoyaCommands);

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

  it.each([
    [
      "a core-state event",
      (status: RuntimeStatusResponse) =>
        useRuntimeEventStore.getState().pushTransientEvent({ kind: "coreState", payload: status }),
    ],
    ["a status read", (status: RuntimeStatusResponse) => useRuntimeEventStore.getState().setCoreState(status)],
  ])("drops the previous session's connections when %s says the core disconnected", async (_source, apply) => {
    vi.useFakeTimers();
    apply(coreStatus("connected"));
    useRuntimeEventStore.getState().setProxyConnections(
      makeConnectionsSnapshot("connection-1", "old.example.com:443", 200, 100),
    );
    // A push still queued for the old core must not land after the disconnect.
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyConnections",
      payload: makeConnectionsSnapshot("connection-2", "late.example.com:443", 300, 100),
    });

    apply(coreStatus("disconnected"));
    await vi.advanceTimersByTimeAsync(20);

    expect(useRuntimeEventStore.getState().proxyConnections).toBeNull();

    // The next session's first push schedules a frame of its own.
    apply(coreStatus("connected"));
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "proxyConnections",
      payload: makeConnectionsSnapshot("connection-3", "new.example.com:443", 10, 5),
    });
    await vi.advanceTimersByTimeAsync(20);

    expect(useRuntimeEventStore.getState().proxyConnections?.connections[0]?.id).toBe("connection-3");
  });

  it("keeps the connection table while the core stays connected", () => {
    useRuntimeEventStore.getState().setCoreState(coreStatus("connected"));
    useRuntimeEventStore.getState().setProxyConnections(cachedConnections);

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "coreState",
      payload: { ...coreStatus("connected"), connectedDurationMs: 5_000 },
    });

    expect(useRuntimeEventStore.getState().proxyConnections).toBe(cachedConnections);
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

  it("coalesces log batches into one frame and keeps when each line was logged", async () => {
    vi.useFakeTimers();
    // Drive the arrival clock with a `Date.now` spy rather than `setSystemTime`:
    // moving the fake system clock after a frame is already scheduled stops that
    // frame from ever firing, which would make this assert a timer quirk instead
    // of the coalescing behaviour.
    const loggedAt = Date.parse("2026-06-01T08:05:00.000Z");
    const arrivedAt = Date.parse("2026-06-01T08:09:10.000Z");
    vi.spyOn(Date, "now").mockReturnValue(arrivedAt);

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "logLines",
      payload: [
        { body: { line: "core started", source: "core" }, id: 1, level: "info", loggedAtMs: loggedAt },
        { body: { line: "inbound/mixed started", source: "core" }, id: 2, level: "info", loggedAtMs: null },
      ],
    });
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "logLines",
      payload: [
        {
          body: { code: { code: "connected" }, detail: null, source: "app" },
          id: 3,
          level: "warn",
          loggedAtMs: loggedAt + 1000,
        },
      ],
    });

    // One `set` per frame, not one per batch: two batches can land between
    // frames.
    expect(useRuntimeEventStore.getState().logLines).toEqual([]);

    await vi.advanceTimersByTimeAsync(20);

    // Lines the backend held back while no Logs panel was open keep the time
    // they were logged, not the panel-open time; arrival time only stands in
    // for a missing stamp.
    expect(useRuntimeEventStore.getState().logLines).toEqual([
      { body: { line: "core started", source: "core" }, id: 1, level: "info", loggedAt },
      {
        body: { line: "inbound/mixed started", source: "core" },
        id: 2,
        level: "info",
        loggedAt: arrivedAt,
      },
      {
        body: { code: { code: "connected" }, detail: null, source: "app" },
        id: 3,
        level: "warn",
        loggedAt: loggedAt + 1000,
      },
    ]);
  });

  it("does not notify subscribers for an empty batch", async () => {
    vi.useFakeTimers();
    const before = useRuntimeEventStore.getState();
    const listener = vi.fn();
    const unsubscribe = useRuntimeEventStore.subscribe(listener);

    useRuntimeEventStore.getState().pushTransientEvent({ kind: "logLines", payload: [] });
    useRuntimeEventStore.getState().pushTransientEvent({ kind: "speedtestResults", payload: [] });
    await vi.advanceTimersByTimeAsync(20);
    unsubscribe();

    expect(listener).not.toHaveBeenCalled();
    expect(useRuntimeEventStore.getState().logLines).toBe(before.logLines);
    expect(useRuntimeEventStore.getState().speedtestResultsByProfileId).toBe(before.speedtestResultsByProfileId);
  });

  it("drops buffered log lines when the log is cleared before the frame runs", async () => {
    vi.useFakeTimers();

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "logLines",
      payload: [{ body: { line: "core started", source: "core" }, id: 1, level: "info", loggedAtMs: null }],
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

  it("stores a valid statistics payload with only its known fields", async () => {
    vi.useFakeTimers();
    const statistics = statisticsSnapshot("profile-a", 3);

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "statistics",
      payload: { ...statistics, extra: "dropped" } as StatisticsSnapshot,
    });

    // Applied on the next frame, like the other once-a-second streams.
    expect(useRuntimeEventStore.getState().statistics).toBeNull();

    await vi.advanceTimersByTimeAsync(20);

    expect(useRuntimeEventStore.getState().statistics).toEqual(statistics);
    expect(useRuntimeEventStore.getState().serverStatsByProfileId).toEqual({
      "profile-a": statistics.serverStat,
    });
  });

  it("applies statistics samples that share a frame once, keeping the latest", async () => {
    vi.useFakeTimers();
    const notify = vi.fn();
    const unsubscribe = useRuntimeEventStore.subscribe(notify);

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "statistics",
      payload: statisticsSnapshot("profile-a", 3),
    });
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "statistics",
      payload: statisticsSnapshot("profile-a", 9),
    });
    await vi.advanceTimersByTimeAsync(20);
    unsubscribe();

    expect(notify).toHaveBeenCalledTimes(1);
    expect(useRuntimeEventStore.getState().statistics?.downloadBytesPerSecond).toBe(9);
  });

  it("keeps each node's totals when samples for two nodes share a frame", async () => {
    vi.useFakeTimers();
    const first = statisticsSnapshot("profile-a", 3);
    const second = statisticsSnapshot("profile-b", 4);

    useRuntimeEventStore.getState().pushTransientEvent({ kind: "statistics", payload: first });
    useRuntimeEventStore.getState().pushTransientEvent({ kind: "statistics", payload: second });
    await vi.advanceTimersByTimeAsync(20);

    expect(useRuntimeEventStore.getState().serverStatsByProfileId).toEqual({
      "profile-a": first.serverStat,
      "profile-b": second.serverStat,
    });
    expect(useRuntimeEventStore.getState().statistics).toEqual(second);
  });

  it("lets the zero sample after traffic stops land last", async () => {
    vi.useFakeTimers();

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "statistics",
      payload: statisticsSnapshot("profile-a", 7),
    });
    await vi.advanceTimersByTimeAsync(20);
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "statistics",
      payload: statisticsSnapshot("profile-a", 0),
    });
    await vi.advanceTimersByTimeAsync(20);

    expect(useRuntimeEventStore.getState().statistics?.downloadBytesPerSecond).toBe(0);
  });

  it("rejects invalid statistics payloads before storing them", async () => {
    vi.useFakeTimers();
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
    await vi.advanceTimersByTimeAsync(20);

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

function coreStatus(state: RuntimeStatusResponse["state"]): RuntimeStatusResponse {
  return {
    activeProfileId: state === "connected" ? "profile-a" : null,
    activeTunBackend: null,
    connectedDurationMs: state === "connected" ? 0 : null,
    mainPid: null,
    prePid: null,
    state,
  };
}

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

function statisticsSnapshot(indexId: string, downloadBytesPerSecond: number): StatisticsSnapshot {
  return {
    activeProfileId: indexId,
    directDownloadBytesPerSecond: 1,
    directUploadBytesPerSecond: 2,
    downloadBytesPerSecond,
    proxyDownloadBytesPerSecond: null,
    proxyUploadBytesPerSecond: 4,
    serverStat: {
      dateNow: 5,
      indexId,
      todayDown: 6,
      todayUp: 7,
      totalDown: null,
      totalUp: 8,
    },
    uploadBytesPerSecond: 0,
  };
}
