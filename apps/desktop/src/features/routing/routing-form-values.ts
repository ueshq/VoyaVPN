import type { RoutingRule, RoutingRuleScope } from "@/ipc/bindings";

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
    domain: textToList(form.domain),
    enabled: form.enabled,
    inboundTags: form.inboundTags,
    ip: textToList(form.ip),
    kind: form.kind,
    network: [form.tcp && "tcp", form.udp && "udp"].filter(Boolean).join(",") || null,
    outbound: emptyToNull(form.outbound),
    port: emptyToNull(form.port),
    process: textToList(form.process),
    protocol: textToList(form.protocol),
    remarks: emptyToNull(form.remarks),
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

function textToList(value: string) {
  const list = value.split(/[\n,]/).flatMap((item) => {
    const trimmed = item.trim();

    return trimmed ? [trimmed] : [];
  });

  return list.length > 0 ? list : null;
}

function emptyToNull(value: string) {
  const trimmed = value.trim();

  return trimmed ? trimmed : null;
}
