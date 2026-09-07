import { describe, expect, it } from "vitest";

import { zodIssuesToErrorMap } from "@/lib/zod-errors";

import {
  routingProfileFieldsSchema,
  routingProfileSchema,
  routingRuleSchema,
} from "./routing-form-schema";

describe("strict routing schemas", () => {
  it("accepts canonical HTTPS sources and rule expressions", () => {
    expect(routingProfileSchema.parse({
      domainStrategy: "AsIs",
      enabled: true,
      remarks: " Voya routing ",
      rules: [{ ...baseRule(), network: "tcp,udp", port: "53, 80-443" }],
      singboxDomainStrategy: "",
      singboxRulesetPath: " rules/default.srs ",
      sourceUrl: "https://routing.example.test/bundle.json",
    })).toMatchObject({
      id: "",
      locked: false,
      remarks: "Voya routing",
      sort: 0,
    });
  });

  // Messages are translation keys, not display strings (see lib/zod-errors.ts).
  it.each([
    ["not a url", "validation.urlInvalid"],
    ["http://routing.example.test/bundle.json", "validation.urlHttps"],
    ["https://user:secret@routing.example.test/bundle.json", "validation.urlCredentials"],
  ])("rejects source %s", (sourceUrl, message) => {
    const result = routingProfileSchema.safeParse({
      domainStrategy: "AsIs",
      enabled: true,
      remarks: "routing",
      rules: [],
      singboxDomainStrategy: "",
      singboxRulesetPath: "",
      sourceUrl,
    });
    expect(result.success).toBe(false);
    if (!result.success) expect(result.error.issues[0]?.message).toBe(message);
  });

  it("allows an empty local source", () => {
    expect(routingProfileSchema.safeParse({
      domainStrategy: "IPIfNonMatch",
      enabled: false,
      remarks: "",
      rules: [],
      singboxDomainStrategy: "prefer_ipv4",
      singboxRulesetPath: "",
      sourceUrl: "",
    }).success).toBe(true);
  });

  it.each([
    ["abc", "validation.port"],
    ["70000", "validation.portRange"],
    ["100-10", "validation.portRange"],
  ])("rejects invalid port expression %s", (port, message) => {
    const result = routingRuleSchema.safeParse({ ...baseRule(), port });
    expect(result.success).toBe(false);
    if (!result.success) expect(result.error.issues.find((issue) => issue.path[0] === "port")?.message).toBe(message);
  });

  it.each(["icmp", "tcp,icmp", ","])("rejects invalid network expression %s", (network) => {
    expect(routingRuleSchema.safeParse({ ...baseRule(), network }).success).toBe(false);
  });

  it.each([null, "", "tcp", "udp", "TCP, udp"])("accepts network expression %s", (network) => {
    expect(routingRuleSchema.safeParse({ ...baseRule(), network }).success).toBe(true);
  });

  it("maps nested Zod issues to stable form field names", () => {
    const result = routingProfileSchema.safeParse({});
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(zodIssuesToErrorMap(result.error)).toMatchObject({
        domainStrategy: expect.any(String),
        enabled: expect.any(String),
        rules: expect.any(String),
        sourceUrl: expect.any(String),
      });
    }
  });

  it("carries a rule set the profile dialog cannot repair through untouched", () => {
    // A rule stored before a stricter validator existed must not make the
    // profile dialog's Save a silent no-op; only profile fields are validated.
    const brokenRule = { ...baseRule(), port: "not-a-port" };
    expect(routingProfileSchema.safeParse({
      domainStrategy: "AsIs",
      enabled: true,
      remarks: "routing",
      rules: [brokenRule],
      singboxDomainStrategy: "",
      singboxRulesetPath: "",
      sourceUrl: "",
    }).success).toBe(false);

    expect(routingProfileFieldsSchema.safeParse({
      domainStrategy: "AsIs",
      enabled: true,
      remarks: "routing",
      singboxDomainStrategy: "",
      singboxRulesetPath: "",
      sourceUrl: "",
    }).success).toBe(true);
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
