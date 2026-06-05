import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { ClashConnectionsSnapshot } from "@/ipc/bindings";

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

  it("clears stale state when fresh Clash traffic arrives", () => {
    useRuntimeEventStore.getState().setClashMonitorFailed("stream failed");

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "clashTraffic",
      payload: { down: 2048, up: 1024 },
    });

    expect(useRuntimeEventStore.getState().clashTraffic).toEqual({ down: 2048, up: 1024 });
    expect(useRuntimeEventStore.getState().clashMonitorStatus).toEqual({
      message: null,
      running: true,
      stale: false,
      state: "running",
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
      message: null,
      running: true,
      stale: false,
      state: "running",
    });
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
