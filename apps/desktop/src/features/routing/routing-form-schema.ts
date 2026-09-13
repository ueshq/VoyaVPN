import { z } from "zod";

import { ruleHasMatcher } from "./rule-match-summary";

// Issue messages are translation keys, not display strings: the dialog renders
// them through `t` (see lib/zod-errors.ts). A literal English message here would
// reach the user untranslated and invisibly to the i18n gate, which only scans
// JSX text.
const nullableText = z.string().trim().nullable();
const nullableStringList = z.array(z.string().trim().min(1, "validation.listItem")).nullable();

export const routingRuleSchema = z
  .object({
    id: z.string().trim().default(""),
    kind: nullableText,
    port: nullableText.superRefine(validatePortExpression),
    network: nullableText,
    inboundTags: nullableStringList,
    outbound: nullableText,
    ip: nullableStringList,
    domain: nullableStringList,
    protocol: nullableStringList,
    process: nullableStringList,
    enabled: z.boolean().default(true),
    remarks: nullableText,
    scope: z.enum(["all", "routing", "dns"]).nullable(),
  })
  .superRefine((rule, context) => {
    // The generator skips a rule without any matcher, so saving one would
    // look like a working rule that silently never applies.
    if (!ruleHasMatcher(rule)) {
      context.addIssue({ code: "custom", message: "validation.routingRuleWithoutMatcher", path: [] });
    }
  });

export type RoutingRulePayload = z.output<typeof routingRuleSchema>;

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
