import type { RoutingRule, RoutingRuleScope, Routing_Serialize } from "@/ipc/bindings";

import { RULE_TYPES } from "./routing-constants";

export type RuleFormState = {
  id?: string;
  domain: string;
  enabled: boolean;
  inboundTags: string;
  ip: string;
  network: string;
  outbound: string;
  port: string;
  process: string;
  protocol: string;
  remarks: string;
  scope: RoutingRuleScope;
  kind: string;
};

export type RoutingFormState = {
  id?: string;
  icon?: string;
  singboxRulesetPath: string;
  domainStrategy: string;
  singboxDomainStrategy: string;
  enabled: boolean;
  isActive?: boolean;
  locked?: boolean;
  remarks: string;
  rules: RoutingRule[];
  sort?: number;
  sourceUrl: string;
};

export function routingToForm(routing: Routing_Serialize | null): RoutingFormState {
  return routing
    ? {
        ...routing,
        domainStrategy: routing.domainStrategy || "AsIs",
      }
    : createDefaultRouting();
}

export function ruleToForm(rule: RoutingRule | null): RuleFormState {
  return {
    id: rule?.id,
    domain: listToText(rule?.domain),
    enabled: rule?.enabled ?? true,
    inboundTags: listToText(rule?.inboundTags),
    ip: listToText(rule?.ip),
    network: rule?.network ?? "",
    outbound: rule?.outbound ?? "proxy",
    port: rule?.port ?? "",
    process: listToText(rule?.process),
    protocol: listToText(rule?.protocol),
    remarks: rule?.remarks ?? "",
    scope: rule?.scope ?? RULE_TYPES.Routing,
    kind: rule?.kind ?? "",
  };
}

export function formToRule(form: RuleFormState): RoutingRule {
  return {
    id: form.id ?? "",
    domain: textToList(form.domain),
    enabled: form.enabled,
    inboundTags: textToList(form.inboundTags),
    ip: textToList(form.ip),
    network: emptyToNull(form.network),
    outbound: emptyToNull(form.outbound),
    port: emptyToNull(form.port),
    process: textToList(form.process),
    protocol: textToList(form.protocol),
    remarks: emptyToNull(form.remarks),
    scope: form.scope,
    kind: emptyToNull(form.kind),
  };
}

function createDefaultRouting(): RoutingFormState {
  return {
    singboxRulesetPath: "",
    domainStrategy: "AsIs",
    singboxDomainStrategy: "",
    enabled: true,
    remarks: "",
    rules: [],
    sourceUrl: "",
  };
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
