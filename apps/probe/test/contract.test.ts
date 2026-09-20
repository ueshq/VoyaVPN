/**
 * The same fixtures the Rust client decodes (`crates/voya-net`), so a field
 * renamed on either side fails both suites.
 */
import { describe, expect, it } from "vitest";

import errorFixture from "../../../tests/probe-contract/error.json";
import requestFixture from "../../../tests/probe-contract/probe.request.json";
import ipv6Fixture from "../../../tests/probe-contract/probe.response.ipv6.json";
import responseFixture from "../../../tests/probe-contract/probe.response.json";
import { handleRequest, parsePorts, type ProbeOutcome } from "../src/probe";

// The fixtures carry documentation addresses (RFC 5737, RFC 3849), which the
// handler refuses to dial, so each request comes from a routable caller of the
// same family.
const ROUTABLE_CALLER: Record<string, string> = {
  "203.0.113.7": "104.16.0.7",
  "2001:db8::7": "2606:4700::7",
};

function shape(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(shape);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, entry]) => [key, shape(entry)]));
  }
  return typeof value;
}

describe("probe wire contract", () => {
  it("accepts the request the client sends", () => {
    expect(parsePorts(JSON.stringify(requestFixture))).toEqual(requestFixture.ports);
  });

  it("answers in the shape the client decodes", async () => {
    for (const fixture of [responseFixture, ipv6Fixture]) {
      const outcomes = new Map(fixture.results.map((result) => [result.port, result.reason as ProbeOutcome]));
      const response = await handleRequest(
        new Request("https://probe.example/v1/probe", {
          method: "POST",
          body: JSON.stringify({ ports: fixture.results.map((result) => result.port) }),
          headers: { "CF-Connecting-IP": ROUTABLE_CALLER[fixture.ip] ?? fixture.ip },
        }),
        { dial: async (_address, port) => outcomes.get(port) ?? "error" },
      );
      const answer = (await response.json()) as typeof fixture;
      expect(shape(answer)).toEqual(shape(fixture));
      expect(answer.family).toBe(fixture.family);
      expect(answer.results.map(({ port, reachable, reason }) => ({ port, reachable, reason }))).toEqual(
        fixture.results.map(({ port, reachable, reason }) => ({ port, reachable, reason })),
      );
    }
  });

  it("reports errors in the shape the client expects", async () => {
    const response = await handleRequest(
      new Request("https://probe.example/v1/probe", {
        method: "POST",
        body: "{}",
        headers: { "CF-Connecting-IP": ROUTABLE_CALLER["203.0.113.7"] },
      }),
      { dial: async () => "error" },
    );
    expect(await response.json()).toEqual(errorFixture);
  });
});
