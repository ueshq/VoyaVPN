import { describe, expect, it } from "vitest";

import type { RoutingRule } from "@/ipc/bindings";

import { ruleHasMatcher, ruleMatchChips } from "./rule-match-summary";

describe("rule match summary", () => {
  it("summarizes each matcher list by its first value and the count of the rest", () => {
    expect(
      ruleMatchChips(
        rule({
          domain: ["domain:alidns.com", "domain:doh.pub", " "],
          ip: ["223.5.5.5"],
          process: [" ", "curl"],
          protocol: ["quic", "tls"],
        }),
      ),
    ).toEqual([
      { field: "domain", first: "domain:alidns.com", kind: "list", more: 1 },
      { field: "ip", first: "223.5.5.5", kind: "list", more: 0 },
      { field: "process", first: "curl", kind: "list", more: 0 },
      { field: "protocol", first: "quic", kind: "list", more: 1 },
    ]);
  });

  it("joins networks and the port, and flags a scope narrower than both", () => {
    expect(ruleMatchChips(rule({ network: "udp", port: "443", scope: "routing" }))).toEqual([
      { kind: "port", network: "UDP", port: "443" },
      { kind: "scope", scope: "routing" },
    ]);
    expect(ruleMatchChips(rule({ network: "tcp, udp", scope: "dns" }))).toEqual([
      { kind: "port", network: "TCP/UDP", port: null },
      { kind: "scope", scope: "dns" },
    ]);
    expect(ruleMatchChips(rule({ port: "80", scope: "all" }))).toEqual([
      { kind: "port", network: null, port: "80" },
    ]);
  });

  it("knows a rule without matchers never applies", () => {
    expect(ruleHasMatcher(rule({ domain: [" "], port: " ", scope: "dns" }))).toBe(false);
    expect(ruleHasMatcher(rule({ inboundTags: ["tun-in"] }))).toBe(true);
    expect(ruleHasMatcher(rule({ network: "tcp" }))).toBe(true);
    expect(ruleHasMatcher(rule({ port: "443" }))).toBe(true);
    expect(ruleHasMatcher(rule({ ip: ["geoip:cn"] }))).toBe(true);
  });
});

function rule(overrides: Partial<RoutingRule>): RoutingRule {
  return {
    domain: null,
    enabled: true,
    id: "rule",
    inboundTags: null,
    ip: null,
    kind: null,
    network: null,
    outbound: "proxy",
    port: null,
    process: null,
    protocol: null,
    remarks: "Rule",
    scope: null,
    ...overrides,
  };
}
