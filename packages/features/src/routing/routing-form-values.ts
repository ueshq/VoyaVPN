import type { RoutingRule, RoutingRuleScope } from "@voya/contracts";
import { splitList, trimToNull } from "@voya/utils/text";

export type RuleFormState = {
  id: string;
  domain: string;
  enabled: boolean;
  ip: string;
  outbound: string;
  port: string;
  process: string;
  protocol: string;
  remarks: string;
  scope: RoutingRuleScope;
  tcp: boolean;
  udp: boolean;
  // The editor has no field for these. They are carried through unchanged, so
  // saving a rule never drops what an older build stored in them.
  inboundTags: string[] | null;
  kind: string | null;
};

export function ruleToForm(rule: RoutingRule | null): RuleFormState {
  const network = parseNetwork(rule?.network);

  return {
    id: rule?.id ?? "",
    domain: listToText(rule?.domain),
    enabled: rule?.enabled ?? true,
    inboundTags: rule?.inboundTags ?? null,
    ip: listToText(rule?.ip),
    kind: rule?.kind ?? null,
    // The generator sends a rule without an outbound through the proxy.
    outbound: rule?.outbound?.trim() || "proxy",
    port: rule?.port ?? "",
    process: listToText(rule?.process),
    protocol: listToText(rule?.protocol),
    remarks: rule?.remarks ?? "",
    // A rule without a scope reaches both the route and the DNS generator.
    scope: rule?.scope ?? "all",
    tcp: network.has("tcp"),
    udp: network.has("udp"),
  };
}

export function formToRule(form: RuleFormState): RoutingRule {
  return {
    id: form.id,
    domain: listOrNull(form.domain),
    enabled: form.enabled,
    inboundTags: form.inboundTags,
    ip: listOrNull(form.ip),
    kind: form.kind,
    network: [form.tcp && "tcp", form.udp && "udp"].filter(Boolean).join(",") || null,
    outbound: trimToNull(form.outbound),
    port: trimToNull(form.port),
    process: listOrNull(form.process),
    protocol: listOrNull(form.protocol),
    remarks: trimToNull(form.remarks),
    scope: form.scope,
  };
}

function parseNetwork(value: string | null | undefined): Set<string> {
  return new Set(
    (value ?? "")
      .split(",")
      .map((item) => item.trim().toLowerCase())
      .filter(Boolean),
  );
}

function listToText(values: string[] | null | undefined) {
  return values?.join("\n") ?? "";
}

/** The contract stores "no entries" as `null`, not as an empty list. */
function listOrNull(value: string) {
  const list = splitList(value);

  return list.length > 0 ? list : null;
}
