import { describe, expect, it } from "vitest";

import { zodIssuesToErrorMap } from "@voya/features/forms/zod-errors";

import { routingRuleSchema } from "./routing-form-schema";

describe("strict routing rule schema", () => {
  it("accepts canonical rule expressions and fills defaults", () => {
    expect(
      routingRuleSchema.parse({
        ...baseRule(),
        domain: ["domain:example.test"],
        network: "tcp,udp",
        port: "53, 80-443",
      }),
    ).toMatchObject({ enabled: true, id: "", port: "53, 80-443" });
  });

  it.each([
    ["abc", "validation.port"],
    ["70000", "validation.portRange"],
    ["0", "validation.portRange"],
    ["100-10", "validation.portRange"],
  ])("rejects invalid port expression %s", (port, message) => {
    const result = routingRuleSchema.safeParse({ ...baseRule(), port });
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error.issues.find((issue) => issue.path[0] === "port")?.message).toBe(message);
    }
  });

  it("rejects a rule without any matcher as a form-level issue", () => {
    const result = routingRuleSchema.safeParse({ ...baseRule(), domain: [], remarks: "Empty" });
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(zodIssuesToErrorMap(result.error)).toEqual({
        form: "validation.routingRuleWithoutMatcher",
      });
    }
  });

  it.each([
    ["inbound tags", { inboundTags: ["tun-in"] }],
    ["a network", { network: "udp" }],
    ["a protocol", { protocol: ["quic"] }],
    ["a process", { process: ["curl"] }],
  ])("counts %s as a matcher", (_label, matcher) => {
    expect(routingRuleSchema.safeParse({ ...baseRule(), ...matcher }).success).toBe(true);
  });

  it("maps a list item issue to a path under its field", () => {
    const result = routingRuleSchema.safeParse({ ...baseRule(), domain: ["ok.test", " "] });
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(zodIssuesToErrorMap(result.error)).toMatchObject({ "domain.1": "validation.listItem" });
    }
  });
});

function baseRule() {
  return {
    domain: null,
    inboundTags: null,
    ip: null,
    kind: null,
    network: null,
    outbound: null,
    port: null,
    process: null,
    protocol: null,
    remarks: null,
    scope: null,
  };
}
