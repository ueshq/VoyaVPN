import { z } from "zod";

import { DOMAIN_STRATEGIES, RULE_TYPES, SINGBOX_DOMAIN_STRATEGIES } from "./routing-constants";

// Issue messages are translation keys, not display strings: the dialogs render
// them through `t` (see lib/zod-errors.ts). A literal English message here would
// reach the user untranslated and invisibly to the i18n gate, which only scans
// JSX text.
const nullableText = z.string().trim().nullable();
const nullableStringList = z.array(z.string().trim().min(1, "validation.listItem")).nullable();

export const routingRuleSchema = z.object({
  id: z.string().trim().default(""),
  kind: nullableText,
  port: nullableText.superRefine(validatePortExpression),
  network: nullableText.superRefine(validateNetworkExpression),
  inboundTags: nullableStringList,
  outbound: nullableText,
  ip: nullableStringList,
  domain: nullableStringList,
  protocol: nullableStringList,
  process: nullableStringList,
  enabled: z.boolean().default(true),
  remarks: nullableText,
  scope: z.union([z.literal(RULE_TYPES.All), z.literal(RULE_TYPES.Routing), z.literal(RULE_TYPES.Dns)]).nullable(),
});

const optionalHttpsUrl = z.string().trim().superRefine(validateHttpsUrl);

export const routingProfileSchema = z.object({
  id: z.string().trim().default(""),
  icon: z.string().default(""),
  singboxRulesetPath: z.string().trim(),
  domainStrategy: z.enum(DOMAIN_STRATEGIES),
  singboxDomainStrategy: z.enum(SINGBOX_DOMAIN_STRATEGIES),
  enabled: z.boolean(),
  locked: z.boolean().default(false),
  remarks: z.string().trim().max(256, "validation.remarksLength"),
  rules: z.array(routingRuleSchema),
  sort: z.number().int().default(0),
  sourceUrl: optionalHttpsUrl,
});

/**
 * Profile-level fields only. The dialog that edits a routing profile must not
 * re-validate the rules it merely carries through: a rule saved by an older
 * build (or imported from a template) that fails a strict validator would make
 * Save a silent no-op with no field to attach the message to.
 */
export const routingProfileFieldsSchema = routingProfileSchema.omit({ rules: true });

export type RoutingFormPayload = z.output<typeof routingProfileSchema>;
export type RoutingRulePayload = z.output<typeof routingRuleSchema>;

function validateHttpsUrl(value: string, context: z.RefinementCtx) {
  if (value === "") {
    return;
  }

  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    context.addIssue({ code: "custom", message: "validation.urlInvalid" });
    return;
  }

  if (parsed.protocol !== "https:") {
    context.addIssue({ code: "custom", message: "validation.urlHttps" });
  }
  if (!parsed.hostname) {
    context.addIssue({ code: "custom", message: "validation.urlHost" });
  }
  if (parsed.username || parsed.password) {
    context.addIssue({ code: "custom", message: "validation.urlCredentials" });
  }
}

function validatePortExpression(value: string | null | undefined, context: z.RefinementCtx) {
  if (!value) {
    return;
  }

  for (const token of value.split(",")) {
    const part = token.trim();
    const match = /^(\d{1,5})(?:-(\d{1,5}))?$/.exec(part);
    if (!match) {
      context.addIssue({ code: "custom", message: "validation.port" });
      return;
    }

    const start = Number(match[1]);
    const end = match[2] ? Number(match[2]) : start;
    if (start > 65535 || end > 65535 || start > end) {
      context.addIssue({ code: "custom", message: "validation.portRange" });
      return;
    }
  }
}

function validateNetworkExpression(value: string | null | undefined, context: z.RefinementCtx) {
  if (!value) {
    return;
  }

  const allowed = new Set(["tcp", "udp"]);
  const values = value.split(",").flatMap((item) => {
    const normalized = item.trim().toLowerCase();

    return normalized ? [normalized] : [];
  });
  if (values.length === 0 || values.some((item) => !allowed.has(item))) {
    context.addIssue({ code: "custom", message: "validation.network" });
  }
}
