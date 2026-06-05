import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useRuntimeEventStore } from "@/ipc/runtime-event-store";

describe("runtime event store", () => {
  beforeEach(() => {
    useRuntimeEventStore.setState({
      clashConnections: null,
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

  it("coalesces Clash connection websocket events into the next frame", async () => {
    vi.useFakeTimers();

    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "clashConnections",
      payload: {
        connections: [
          {
            chains: ["Proxy"],
            connectionType: "HTTP",
            destination: "93.184.216.34:443",
            download: 200,
            host: "example.com:443",
            id: "connection-1",
            network: "tcp",
            process: "browser",
            processPath: "/usr/bin/browser",
            rule: "MATCH",
            rulePayload: null,
            source: "127.0.0.1:53000",
            start: "2026-06-01T00:00:00Z",
            upload: 100,
          },
        ],
        downloadTotal: 200,
        uploadTotal: 100,
      },
    });
    useRuntimeEventStore.getState().pushTransientEvent({
      kind: "clashConnections",
      payload: {
        connections: [
          {
            chains: ["Direct"],
            connectionType: "HTTP",
            destination: "93.184.216.34:443",
            download: 400,
            host: "latest.example.com:443",
            id: "connection-2",
            network: "tcp",
            process: "browser",
            processPath: "/usr/bin/browser",
            rule: "MATCH",
            rulePayload: null,
            source: "127.0.0.1:53000",
            start: "2026-06-01T00:00:00Z",
            upload: 300,
          },
        ],
        downloadTotal: 400,
        uploadTotal: 300,
      },
    });

    expect(useRuntimeEventStore.getState().clashConnections).toBeNull();

    await vi.advanceTimersByTimeAsync(20);

    const snapshot = useRuntimeEventStore.getState().clashConnections;

    expect(snapshot?.connections[0]?.host).toBe("latest.example.com:443");
    expect(snapshot?.downloadTotal).toBe(400);
    expect(useRuntimeEventStore.getState().lastTransientEvent?.kind).toBe("clashConnections");
  });
});
