import { expect, it } from "vite-plus/test";
import type { RoutingRule } from "@voya/contracts";
import { i18next } from "@voya/i18n";
import { ruleMatchSummary } from "./rule-summary";

const rule: RoutingRule = { id: "r", remarks: null, enabled: false, scope: "dns", kind: null, port: null, network: null, inboundTags: null, outbound: "direct", domain: null, ip: null, protocol: null, process: null };
it("retains exact matcher expressions, including rule sets and unknown kinds", () => {
  const lines = ruleMatchSummary({ ...rule, domain: ["regexp:^example\\.com$", "rule-set:geosite-cn"], ip: ["10.0.0.0/8"], port: "443", network: "tcp", protocol: ["tls"], process: ["app"], inboundTags: ["tun"], kind: "future-kind" }, i18next.t.bind(i18next));
  expect(lines).toHaveLength(8);
  expect(lines.join("\n")).toContain("regexp:^example\\.com$");
  expect(lines.join("\n")).toContain("rule-set:geosite-cn");
  expect(lines.join("\n")).toContain("future-kind");
});
it("does not invent a destination or matcher for empty conditions", () => {
  const lines = ruleMatchSummary(rule, i18next.t.bind(i18next));
  expect(lines).toHaveLength(1);
  expect(lines[0]).not.toContain("direct");
});
