import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { ClashConnectionsSnapshot, StatisticsSnapshot } from "@/ipc/bindings";

const initialMonitorStatus = {
  message: null,
  running: false,
  stale: true,
  state: "stopped" as const,
};

const cachedConnections: ClashConnectionsSnapshot = {
  connections: [],
  downloadTotal: 200,
  uploadTotal: 100,
};

describe("runtime event store", () => {
  beforeEach(() => {
    useRuntimeEventStore.setState({
      clashConnections: null,
      clashMonitorStatus: initialMonitorStatus,
      clashTraffic: null,
      lastTransientEvent: null,
      logLines: [],
      serverStatsByProfileId: {},
      statistics: null,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("stores Clash traffic websocket events", () => {
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "clashTraffic",
      payload: { down: 2048, up: 1024 },
    });

    expect(useRuntimeEventStore.getState().clashTraffic).toEqual({ down: 2048, up: 1024 });
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("clashTraffic");
  });

  it("sets Clash monitor lifecycle state through store actions", () => {
    useRuntimeEventStore.getState().setClashMonitorRunning();

    expect(useRuntimeEventStore.getState().clashMonitorStatus).toEqual({
      message: null,
      running: true,
      stale: false,
      state: "running",
    });

    useRuntimeEventStore.getState().setClashMonitorStarting("connecting");

    expect(useRuntimeEventStore.getState().clashMonitorStatus).toEqual({
      message: "connecting",
      running: false,
      stale: false,
      state: "starting",
    });

    useRuntimeEventStore.getState().setClashMonitorStopped();

    expect(useRuntimeEventStore.getState().clashMonitorStatus).toEqual({
      message: null,
      running: false,
      stale: true,
      state: "stopped",
    });

    useRuntimeEventStore.getState().setClashMonitorFailed("start failed");

    expect(useRuntimeEventStore.getState().clashMonitorStatus).toEqual({
      message: "start failed",
      running: false,
      stale: true,
      state: "failed",
    });
  });

  it("only clears stale state when fresh Clash traffic arrives", () => {
    useRuntimeEventStore.getState().setClashMonitorFailed("stream failed");

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "clashTraffic",
      payload: { down: 2048, up: 1024 },
    });

    expect(useRuntimeEventStore.getState().clashTraffic).toEqual({ down: 2048, up: 1024 });
    expect(useRuntimeEventStore.getState().clashMonitorStatus).toEqual({
      message: "stream failed",
      running: false,
      stale: false,
      state: "failed",
    });
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("clashTraffic");
  });

  it("does not promote stopped monitor status when late Clash traffic arrives", () => {
    useRuntimeEventStore.getState().setClashMonitorStopped("monitor stopped");

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "clashTraffic",
      payload: { down: 2048, up: 1024 },
    });

    expect(useRuntimeEventStore.getState().clashTraffic).toEqual({ down: 2048, up: 1024 });
    expect(useRuntimeEventStore.getState().clashMonitorStatus).toEqual({
      message: "monitor stopped",
      running: false,
      stale: false,
      state: "stopped",
    });
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("clashTraffic");
  });

  it("marks stopped monitor status stale while preserving Clash snapshots", () => {
    useRuntimeEventStore.getState().setClashTraffic({ down: 2048, up: 1024 });
    useRuntimeEventStore.getState().setClashConnections(cachedConnections);

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "clashMonitorStatus",
      payload: { state: "stopped", running: false, stale: true, message: null },
    });

    expect(useRuntimeEventStore.getState().clashTraffic).toEqual({ down: 2048, up: 1024 });
    expect(useRuntimeEventStore.getState().clashConnections).toEqual({
      connections: [],
      downloadTotal: 200,
      uploadTotal: 100,
    });
    expect(useRuntimeEventStore.getState().clashMonitorStatus).toEqual({
      message: null,
      running: false,
      stale: true,
      state: "stopped",
    });
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("clashMonitorStatus");
  });

  it("marks failed monitor status stale with a message while preserving Clash snapshots", () => {
    useRuntimeEventStore.getState().setClashTraffic({ down: 2048, up: 1024 });
    useRuntimeEventStore.getState().setClashConnections(cachedConnections);

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "clashMonitorStatus",
      payload: { state: "failed", running: false, stale: true, message: "monitor failed" },
    });

    expect(useRuntimeEventStore.getState().clashTraffic).toEqual({ down: 2048, up: 1024 });
    expect(useRuntimeEventStore.getState().clashConnections).toEqual(cachedConnections);
    expect(useRuntimeEventStore.getState().clashMonitorStatus).toEqual({
      message: "monitor failed",
      running: false,
      stale: true,
      state: "failed",
    });
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("clashMonitorStatus");
  });

  it("coalesces Clash connection websocket events into the next frame", async () => {
    vi.useFakeTimers();

    useRuntimeEventStore.getState().setClashMonitorFailed("stream failed");

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "clashConnections",
      payload: makeConnectionsSnapshot("connection-1", "example.com:443", 200, 100),
    });
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "clashConnections",
      payload: makeConnectionsSnapshot("connection-2", "latest.example.com:443", 400, 300, ["Direct"]),
    });

    expect(useRuntimeEventStore.getState().clashConnections).toBeNull();
    expect(useRuntimeEventStore.getState().clashMonitorStatus.stale).toBe(true);

    await vi.advanceTimersByTimeAsync(20);

    const snapshot = useRuntimeEventStore.getState().clashConnections;

    expect(snapshot?.connections[0]?.host).toBe("latest.example.com:443");
    expect(snapshot?.downloadTotal).toBe(400);
    expect(useRuntimeEventStore.getState().clashMonitorStatus).toEqual({
      message: "stream failed",
      running: false,
      stale: false,
      state: "failed",
    });
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("clashConnections");
  });

  it("rejects invalid statistics payloads before storing them", () => {
    const invalidStatistics = {
      activeProfileId: "profile-a",
      directDownloadBytesPerSecond: 0,
      directUploadBytesPerSecond: 0,
      downloadBytesPerSecond: 0,
      proxyDownloadBytesPerSecond: 0,
      proxyUploadBytesPerSecond: Number.NaN,
      serverStat: { IndexId: "profile-a", TotalUp: 1 },
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

  it("does not let invalid Clash connection payloads replace a queued valid frame", async () => {
    vi.useFakeTimers();

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "clashConnections",
      payload: makeConnectionsSnapshot("connection-1", "valid.example.com:443", 200, 100),
    });
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "clashConnections",
      payload: {
        connections: [],
        downloadTotal: -1,
        uploadTotal: 0,
      } as ClashConnectionsSnapshot,
    });

    await vi.advanceTimersByTimeAsync(20);

    expect(useRuntimeEventStore.getState().clashConnections?.connections[0]?.host).toBe("valid.example.com:443");
    expect(useRuntimeEventStore.getState().clashConnections?.downloadTotal).toBe(200);
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("clashConnections");
  });
});

function makeConnectionsSnapshot(
  id: string,
  host: string,
  downloadTotal: number,
  uploadTotal: number,
  chains = ["Proxy"],
): ClashConnectionsSnapshot {
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
