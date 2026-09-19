import { describe, expect, it } from "vitest";

import type { ProxyConnectionItem } from "@/ipc/bindings";
import { arrangeConnections, connectionSearchHay } from "./connection-display";

function makeConnection(overrides: Partial<ProxyConnectionItem> = {}): ProxyConnectionItem {
  return {
    chains: ["Proxy", "Tokyo"],
    connectionType: "HTTP",
    destination: "93.184.216.34:443",
    download: 100,
    host: "example.com:443",
    id: "connection-1",
    network: "tcp",
    process: "browser",
    processPath: "/usr/bin/browser",
    rule: "MATCH",
    rulePayload: null,
    source: "127.0.0.1:53000",
    start: "2026-06-01T00:00:00Z",
    upload: 50,
    ...overrides,
  };
}

describe("connectionSearchHay", () => {
  it("joins every searchable field, lowercased", () => {
    const hay = connectionSearchHay(makeConnection());

    expect(hay).toBe(
      "example.com:443 127.0.0.1:53000 93.184.216.34:443 browser /usr/bin/browser match tcp http proxy tokyo",
    );
  });

  it("skips null fields instead of leaking them into the haystack", () => {
    const hay = connectionSearchHay(
      makeConnection({ process: null, processPath: null, rule: null, rulePayload: null, connectionType: null }),
    );

    expect(hay).toBe("example.com:443 127.0.0.1:53000 93.184.216.34:443 tcp proxy tokyo");
  });

  it("spreads the outbound chains so node names stay searchable", () => {
    const hay = connectionSearchHay(makeConnection({ chains: ["Travel · Auto", "Singapore"] }));

    expect(hay).toContain("travel · auto singapore");
  });
});

describe("arrangeConnections", () => {
  const alpha = makeConnection({
    chains: ["Proxy", "Alpha"],
    host: "alpha.example.com:443",
    id: "alpha",
    upload: 10,
    download: 20,
  });
  const beta = makeConnection({
    chains: ["Direct"],
    host: "beta.example.com:443",
    id: "beta",
    process: "updater",
    upload: 500,
    download: 1000,
  });

  it("returns the snapshot itself when neither searching nor sorting", () => {
    const connections = [alpha, beta];

    expect(arrangeConnections(connections, { routeTexts: null, search: null, sort: null })).toBe(connections);
  });

  it("filters against the precomputed haystacks", () => {
    const connections = [alpha, beta];
    const hays = connections.map(connectionSearchHay);

    expect(arrangeConnections(connections, { routeTexts: null, search: { hays, needle: "updater" }, sort: null })).toEqual([beta]);
    expect(arrangeConnections(connections, { routeTexts: null, search: { hays, needle: "alpha" }, sort: null })).toEqual([alpha]);
    expect(arrangeConnections(connections, { routeTexts: null, search: { hays, needle: "tokyo" }, sort: null })).toEqual([]);
  });

  it("sorts by the precomputed route text and combined traffic in both directions", () => {
    const connections = [alpha, beta];
    const routeTexts = ["Alpha", "Direct"];

    const byRoute = arrangeConnections(connections, { routeTexts, search: null, sort: { column: "route", ascending: true } });
    const byTraffic = arrangeConnections(connections, {
      routeTexts: null,
      search: null,
      sort: { column: "traffic", ascending: false },
    });

    expect(byRoute.map((connection) => connection.id)).toEqual(["alpha", "beta"]);
    expect(byTraffic.map((connection) => connection.id)).toEqual(["beta", "alpha"]);
  });

  it("searches and sorts together", () => {
    const gamma = makeConnection({
      chains: ["Proxy", "Gamma"],
      host: "gamma.example.com:443",
      id: "gamma",
      upload: 900,
      download: 900,
    });
    const connections = [alpha, beta, gamma];
    const hays = connections.map(connectionSearchHay);

    const rows = arrangeConnections(connections, {
      routeTexts: null,
      search: { hays, needle: "example.com" },
      sort: { column: "traffic", ascending: true },
    });

    expect(rows.map((connection) => connection.id)).toEqual(["alpha", "beta", "gamma"]);
  });

  it("keeps a ten-thousand row search-plus-route sort inside the per-second push budget", () => {
    // The whole table re-arranges on every ~1s websocket push. Guard the
    // precomputed-key shape: re-deriving route labels per comparison would push
    // this well past the budget. Mirrors the server-table speedtest budget test.
    const connections = Array.from({ length: 10_000 }, (_, index) =>
      makeConnection({
        chains: ["Proxy", `Node ${index % 64}`],
        host: `host-${index}.example.com:443`,
        id: `connection-${index}`,
      }),
    );
    const hays = connections.map(connectionSearchHay);
    const routeTexts = connections.map((connection) => connection.chains[1] ?? "");

    const started = performance.now();
    for (let round = 0; round < 10; round += 1) {
      arrangeConnections(connections, {
        routeTexts,
        search: { hays, needle: "example.com" },
        sort: { column: "route", ascending: true },
      });
    }

    expect(performance.now() - started).toBeLessThan(1_000);
  });
});
