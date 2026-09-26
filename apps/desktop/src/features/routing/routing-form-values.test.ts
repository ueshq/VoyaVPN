import { describe, expect, it } from "vitest";

import type { RoutingRule } from "@voya/contracts";

import { formToRule, ruleToForm } from "./routing-form-values";

describe("routing rule form transformations", () => {
  it("drafts a new rule that proxies and applies to routing and DNS", () => {
    expect(ruleToForm(null)).toEqual({
      domain: "",
      enabled: true,
      id: "",
      inboundTags: null,
      ip: "",
      kind: null,
      outbound: "proxy",
      port: "",
      process: "",
      protocol: "",
      remarks: "",
      scope: "all",
      tcp: false,
      udp: false,
    });
  });

  it("projects a stored rule onto editable text and network switches", () => {
    expect(ruleToForm(rule())).toEqual({
      domain: "domain:example.test\nexample.org",
      enabled: false,
      id: "rule-a",
      inboundTags: ["mixed-in"],
      ip: "1.1.1.1\n8.8.8.8",
      kind: "field",
      outbound: "direct",
      port: "53,80-90",
      process: "curl\nwget",
      protocol: "dns\nhttp",
      remarks: "Direct services",
      scope: "routing",
      tcp: true,
      udp: true,
    });
  });

  it("reads a missing outbound as the proxy and a missing scope as both", () => {
    const form = ruleToForm({ ...rule(), network: " UDP ", outbound: " ", scope: null });

    expect(form).toMatchObject({ outbound: "proxy", scope: "all", tcp: false, udp: true });
  });

  it("canonicalizes lists, blank values and network switches for the strict DTO", () => {
    expect(
      formToRule({
        ...ruleToForm(null),
        domain: " example.test,example.org\n ",
        inboundTags: ["tun-in"],
        ip: " 1.1.1.1 ",
        kind: "field",
        outbound: " ",
        port: " 80-90 ",
        process: "curl,wget",
        protocol: "",
        remarks: " ",
        tcp: true,
      }),
    ).toEqual({
      domain: ["example.test", "example.org"],
      enabled: true,
      id: "",
      inboundTags: ["tun-in"],
      ip: ["1.1.1.1"],
      kind: "field",
      network: "tcp",
      outbound: null,
      port: "80-90",
      process: ["curl", "wget"],
      protocol: null,
      remarks: null,
      scope: "all",
    });
    expect(formToRule(ruleToForm(null)).network).toBeNull();
  });

  it("round-trips a canonical rule, including the fields the editor does not show", () => {
    expect(formToRule(ruleToForm(rule()))).toEqual(rule());
  });
});

function rule(): RoutingRule {
  return {
    domain: ["domain:example.test", "example.org"],
    enabled: false,
    id: "rule-a",
    inboundTags: ["mixed-in"],
    ip: ["1.1.1.1", "8.8.8.8"],
    kind: "field",
    network: "tcp,udp",
    outbound: "direct",
    port: "53,80-90",
    process: ["curl", "wget"],
    protocol: ["dns", "http"],
    remarks: "Direct services",
    scope: "routing",
  };
}
