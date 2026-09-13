import { describe, expect, it } from "vitest";
import { i18next } from "@voya/i18n";

import type { RoutingRule } from "@/ipc/bindings";

import { connectionRuleText } from "./connection-rule";

const t = i18next.t.bind(i18next);

function rule(remarks: string, overrides: Partial<RoutingRule> = {}): RoutingRule {
  return {
    domain: null,
    enabled: true,
    id: remarks,
    inboundTags: null,
    ip: null,
    kind: null,
    network: null,
    outbound: "proxy",
    port: null,
    process: null,
    protocol: null,
    remarks,
    scope: "routing",
    ...overrides,
  };
}

describe("connection rule text", () => {
  const rules = [
    rule("Off", { domain: ["domain:example.com"], enabled: false }),
    rule("voya:cn-direct", { domain: ["geosite:cn"], ip: ["geoip:cn"] }),
    rule("Work", { domain: ["domain:example.com"] }),
  ];

  it("names the rule whose matcher the core reports, keeping the core text", () => {
    expect(
      connectionRuleText({ rule: "rule_set=[geosite-cn geoip-cn] => route(direct)", rulePayload: "" }, rules, t),
    ).toBe("China sites direct\nrule_set=[geosite-cn geoip-cn] => route(direct)");
    expect(connectionRuleText({ rule: "domain_suffix", rulePayload: "example.com" }, rules, t)).toBe(
      "Work\ndomain_suffix · example.com",
    );
  });

  it("explains an unmatched connection and leaves unknown rules as reported", () => {
    expect(connectionRuleText({ rule: "final", rulePayload: null }, rules, t)).toBe(
      "No rule matched (default route)",
    );
    expect(connectionRuleText({ rule: "ip_cidr=[10.0.0.0/8]", rulePayload: null }, null, t)).toBe(
      "ip_cidr=[10.0.0.0/8]",
    );
    expect(connectionRuleText({ rule: null, rulePayload: null }, rules, t)).toBe("");
  });
});
