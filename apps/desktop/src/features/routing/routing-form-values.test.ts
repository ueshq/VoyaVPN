import { describe, expect, it } from "vitest";

import type { RoutingRule, Routing_Serialize } from "@/ipc/bindings";

import { formToRule, routingToForm, ruleToForm } from "./routing-form-values";

describe("routing form transformations", () => {
  it("creates a clean Voya routing draft", () => {
    expect(routingToForm(null)).toEqual({
      enabled: true,
      remarks: "",
      rules: [],
      singboxDomainStrategy: "",
      singboxRulesetPath: "",
    });
  });

  it("preserves every semantic routing field", () => {
    const routing: Routing_Serialize = {
      enabled: false,
      icon: "route",
      id: "routing-a",
      isActive: true,
      locked: true,
      remarks: "Routing A",
      rules: [rule()],
      singboxDomainStrategy: "ipv4_only",
      singboxRulesetPath: "/rules",
      sort: 4,
    };

    expect(routingToForm(routing)).toEqual(routing);
  });

  it("creates rule defaults and projects nullable values to editable text", () => {
    expect(ruleToForm(null)).toEqual({
      domain: "",
      enabled: true,
      inboundTags: "",
      ip: "",
      kind: "",
      network: "",
      outbound: "proxy",
      port: "",
      process: "",
      protocol: "",
      remarks: "",
      scope: "routing",
    });
    expect(ruleToForm(rule())).toMatchObject({
      domain: "domain:example.test\nexample.org",
      inboundTags: "mixed-in\ntun-in",
      ip: "1.1.1.1\n8.8.8.8",
      process: "curl\nwget",
      protocol: "dns\nhttp",
    });
  });

  it("canonicalizes lists and empty optional values for the strict DTO", () => {
    expect(formToRule({
      domain: " example.test,example.org\n ",
      enabled: true,
      inboundTags: "",
      ip: " 1.1.1.1 ",
      kind: "  field ",
      network: " tcp,udp ",
      outbound: " ",
      port: " 80-90 ",
      process: "curl,wget",
      protocol: "dns",
      remarks: " ",
      scope: "all",
    })).toEqual({
      domain: ["example.test", "example.org"],
      enabled: true,
      id: "",
      inboundTags: null,
      ip: ["1.1.1.1"],
      kind: "field",
      network: "tcp,udp",
      outbound: null,
      port: "80-90",
      process: ["curl", "wget"],
      protocol: ["dns"],
      remarks: null,
      scope: "all",
    });
  });
});

function rule(): RoutingRule {
  return {
    domain: ["domain:example.test", "example.org"],
    enabled: false,
    id: "rule-a",
    inboundTags: ["mixed-in", "tun-in"],
    ip: ["1.1.1.1", "8.8.8.8"],
    kind: "field",
    network: "tcp,udp",
    outbound: "direct",
    port: "53,80-90",
    process: ["curl", "wget"],
    protocol: ["dns", "http"],
    remarks: "Direct services",
    scope: "all",
  };
}
