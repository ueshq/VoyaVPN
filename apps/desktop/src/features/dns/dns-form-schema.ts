import { z } from "zod";

import type { DnsSettings } from "@/ipc/bindings";

export const DNS_STRATEGIES = ["", "AsIs", "UseIP", "UseIPv4", "UseIPv6", "ForceIPv4", "ForceIPv6"] as const;

// Issue messages are translation keys (see lib/zod-errors.ts); the DNS pane
// renders them through `t`, so nothing here may be a display string.
const nullableText = z.string().nullable();

export const dnsSettingsSchema: z.ZodType<DnsSettings> = z.object({
  addCommonHosts: z.boolean().nullable(),
  fakeIp: z.boolean().nullable(),
  globalFakeIp: z.boolean().nullable(),
  blockBindingQuery: z.boolean().nullable(),
  direct: nullableText,
  remote: nullableText,
  bootstrap: nullableText,
  directStrategy: z.enum(DNS_STRATEGIES).nullable(),
  proxyStrategy: z.enum(DNS_STRATEGIES).nullable(),
  hosts: nullableText.superRefine(validateHosts),
  directExpectedIps: nullableText.superRefine(validateExpectedIps),
});

function validateHosts(value: string | null | undefined, context: z.RefinementCtx) {
  if (!value) {
    return;
  }

  value.split(/\r?\n/).forEach((line) => {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) {
      return;
    }
    if (trimmed.split(/\s+/).length < 2) {
      context.addIssue({ code: "custom", message: "validation.hostsLine" });
    }
  });
}

function validateExpectedIps(value: string | null | undefined, context: z.RefinementCtx) {
  if (!value) {
    return;
  }

  if (
    value
      .split(",")
      .map((part) => part.trim())
      .some((part) => part !== "" && /\s/.test(part))
  ) {
    context.addIssue({ code: "custom", message: "validation.expectedIps" });
  }
}
