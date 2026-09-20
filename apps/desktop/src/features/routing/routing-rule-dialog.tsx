import { useId, useState } from "react";
import type * as React from "react";
import { Route } from "lucide-react";

import type { TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { Disclosure } from "@voya/ui/components/disclosure";
import {
  CheckboxField,
  FieldLayout,
  SelectField,
  TextAreaField,
  TextField,
} from "@voya/ui/components/form-fields";

import type { RoutingRule, RoutingRuleScope } from "@/ipc/bindings";
import { validationFieldErrors } from "@voya/client/messages";
import {
  translateFieldErrors,
  zodIssuesToErrorMap,
  type FieldErrorMap,
} from "@voya/features/forms/zod-errors";

import { OUTBOUND_LABEL_KEYS, type RuleGroupOutbound, appendMatcherLine, describeOutbound, groupOutboundValue } from "@voya/features/routing/rule-outbound";
import { RULE_SCOPE_LABEL_KEYS } from "@voya/features/routing/routing-constants";
import { routingRuleSchema, type RoutingRulePayload } from "@voya/features/routing/routing-form-schema";
import { formToRule, ruleToForm, type RuleFormState } from "@voya/features/routing/routing-form-values";
import { sentinelLabelKey } from "@voya/features/routing/sentinel-rules";
import type { RuleSaveError } from "@voya/features/routing/use-routing-screen";

/** Fields with an inline error slot; any other issue is reported above the footer. */
const RENDERED_FIELDS = new Set([
  "domain",
  "ip",
  "network",
  "outbound",
  "port",
  "process",
  "protocol",
  "remarks",
  "scope",
]);

export function RoutingRuleDialog({
  mode,
  nodeNames,
  onOpenChange,
  onSubmit,
  open,
  processRulesSupported = true,
  groupOutbounds = [],
  rule,
  submitError = null,
}: {
  /** Policy groups a rule can target; `null` while the list loads. */
  groupOutbounds?: readonly RuleGroupOutbound[] | null;
  mode: "create" | "edit";
  /** Node remarks a rule can target; `null` while the node list loads. */
  nodeNames: ReadonlySet<string> | null;
  onOpenChange: (open: boolean) => void;
  onSubmit: (rule: RoutingRulePayload) => Promise<void>;
  open: boolean;
  /**
   * The macOS NetworkExtension tunnel cannot match apps, so the field is hidden
   * there. A stored value is carried through unchanged.
   */
  processRulesSupported?: boolean;
  rule: RoutingRule | null;
  /** The backend's refusal of the last save; the editor stays open to show it. */
  submitError?: RuleSaveError | null;
}) {
  const { t } = useI18n();
  const networkId = useId();
  const [form, setForm] = useState(() => ruleToForm(rule));
  const [fieldErrors, setFieldErrors] = useState<FieldErrorMap>({});
  const errors = {
    // A backend refusal shows until the next save; a local check on the same
    // field takes precedence.
    ...(submitError ? validationFieldErrors(t, submitError.issues) : {}),
    ...translateFieldErrors(t, fieldErrors),
  };
  const formError =
    Object.entries(errors).find(([field]) => !RENDERED_FIELDS.has(field))?.[1] ??
    (submitError?.issues.length === 0 ? submitError.message : undefined);
  // Managed rules are found by their reserved remarks; renaming one would
  // silently detach it from what the app knows about it.
  const managedLabel = sentinelLabelKey(rule?.remarks);

  function update<Key extends keyof RuleFormState>(key: Key, value: RuleFormState[Key]) {
    setForm((current) => ({ ...current, [key]: value }));
  }

  async function submitForm(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const parsed = routingRuleSchema.safeParse(formToRule(form));
    if (!parsed.success) {
      setFieldErrors(zodIssuesToErrorMap(parsed.error));
      return;
    }
    setFieldErrors({});
    await onSubmit(parsed.data);
  }

  const outboundOptions: Array<{ label: string; value: string }> = [
    ...Object.entries(OUTBOUND_LABEL_KEYS).map(([value, labelKey]) => ({
      label: t(labelKey),
      value,
    })),
    ...(groupOutbounds ?? []).map(({ id, name }) => ({
      label: t("panes.routing.outboundGroup", { name }),
      value: groupOutboundValue(id),
    })),
    ...[...(nodeNames ?? [])].map((name) => ({ label: name, value: name })),
  ];
  if (!outboundOptions.some((option) => option.value === form.outbound)) {
    // A node or group the rule names stays selectable even when it no longer
    // exists, so opening the editor never retargets the rule behind the
    // user's back.
    const target = describeOutbound(form.outbound, nodeNames, groupOutbounds);
    outboundOptions.push({
      label:
        target.kind === "missingGroup"
          ? t("panes.routing.outboundGroupMissing")
          : target.kind === "missing"
            ? t("panes.routing.outboundMissing", { name: form.outbound })
            : form.outbound,
      value: form.outbound,
    });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <ScrollableDialogContent closeLabel={t("actions.close")} width="54rem">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Route className="size-4" aria-hidden="true" />
            {t(mode === "edit" ? "panes.routing.editRule" : "panes.routing.createRule")}
          </DialogTitle>
          <DialogDescription className="sr-only">{t("panes.routing.ruleEditor")}</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <form
            className="grid min-h-0 gap-4"
            id="routing-rule-form"
            onSubmit={(event) => void submitForm(event)}
          >
            <div className="grid gap-3 sm:grid-cols-2">
              <TextField
                disabled={managedLabel !== null}
                error={errors.remarks}
                label={t("panes.routing.remarks")}
                onChange={(value) => update("remarks", value)}
                value={managedLabel ? t(managedLabel) : form.remarks}
              />
              <SelectField
                error={errors.outbound}
                label={t("panes.routing.outbound")}
                onChange={(value) => update("outbound", value)}
                options={outboundOptions}
                value={form.outbound}
              />
            </div>
            <MatcherPresets
              onAdd={(line) => update("domain", appendMatcherLine(form.domain, line))}
              presets={DOMAIN_PRESETS}
              value={form.domain}
            />
            <TextAreaField
              description={t("panes.routing.domainHelp")}
              error={errors.domain}
              label={t("panes.routing.domain")}
              onChange={(value) => update("domain", value)}
              value={form.domain}
            />
            <MatcherPresets
              onAdd={(line) => update("ip", appendMatcherLine(form.ip, line))}
              presets={IP_PRESETS}
              value={form.ip}
            />
            <TextAreaField
              description={t("panes.routing.ipHelp")}
              error={errors.ip}
              label="IP"
              onChange={(value) => update("ip", value)}
              value={form.ip}
            />
            <div className="grid gap-3 sm:grid-cols-2">
              <TextField
                description={t("panes.routing.portHelp")}
                error={errors.port}
                label={t("panes.routing.port")}
                onChange={(value) => update("port", value)}
                value={form.port}
              />
              <FieldLayout
                description={t("panes.routing.networkHelp")}
                error={errors.network}
                group
                id={networkId}
                label={t("panes.routing.network")}
              >
                <div className="flex h-control items-center gap-5">
                  <CheckboxField
                    checked={form.tcp}
                    label="TCP"
                    onChange={(checked) => update("tcp", checked)}
                  />
                  <CheckboxField
                    checked={form.udp}
                    label="UDP"
                    onChange={(checked) => update("udp", checked)}
                  />
                </div>
              </FieldLayout>
            </div>
            {processRulesSupported ? (
              <TextAreaField
                description={t("panes.routing.processHelp")}
                error={errors.process}
                label={t("panes.routing.process")}
                onChange={(value) => update("process", value)}
                value={form.process}
              />
            ) : null}
            <Disclosure
              invalid={Boolean(errors.scope ?? errors.protocol)}
              title={t("panes.routing.advanced")}
            >
              <SelectField
                description={t("panes.routing.scopeHelp")}
                error={errors.scope}
                label={t("panes.routing.ruleScope")}
                onChange={(value) => update("scope", value as RoutingRuleScope)}
                options={Object.entries(RULE_SCOPE_LABEL_KEYS).map(([value, labelKey]) => ({
                  label: t(labelKey),
                  value,
                }))}
                value={form.scope}
              />
              <TextAreaField
                description={t("panes.routing.protocolHelp")}
                error={errors.protocol}
                label={t("panes.routing.protocol")}
                onChange={(value) => update("protocol", value)}
                value={form.protocol}
              />
            </Disclosure>
            {formError ? (
              <Alert variant="destructive">
                <AlertDescription>{formError}</AlertDescription>
              </Alert>
            ) : null}
          </form>
        </DialogBody>
        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} type="button" variant="outline">
            {t("actions.cancel")}
          </Button>
          <Button form="routing-rule-form" type="submit">
            {t("actions.save")}
          </Button>
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}

/** The rule sets shipped with the app, offered as one-click matchers. */
const DOMAIN_PRESETS = [
  { labelKey: "panes.routing.presets.cnSites", line: "geosite:cn" },
  { labelKey: "panes.routing.presets.google", line: "geosite:google" },
  { labelKey: "panes.routing.presets.ads", line: "geosite:category-ads-all" },
] as const satisfies ReadonlyArray<{ labelKey: TranslationKey; line: string }>;

const IP_PRESETS = [
  { labelKey: "panes.routing.presets.cnIps", line: "geoip:cn" },
  { labelKey: "panes.routing.presets.lanIps", line: "geoip:private" },
] as const satisfies ReadonlyArray<{ labelKey: TranslationKey; line: string }>;

function MatcherPresets({
  onAdd,
  presets,
  value,
}: {
  onAdd: (line: string) => void;
  presets: ReadonlyArray<{ labelKey: TranslationKey; line: string }>;
  value: string;
}) {
  const { t } = useI18n();
  const lines = new Set(value.split(/\r?\n/).map((line) => line.trim()));
  return (
    <div className="-mb-2 flex flex-wrap items-center gap-1.5 text-xs text-muted-foreground">
      <span>{t("panes.routing.presets.label")}</span>
      {presets.map(({ labelKey, line }) => (
        <Button
          aria-pressed={lines.has(line)}
          className="h-6 px-2 text-xs"
          disabled={lines.has(line)}
          key={line}
          onClick={() => onAdd(line)}
          size="sm"
          title={line}
          type="button"
          variant="outline"
        >
          {t(labelKey)}
        </Button>
      ))}
    </div>
  );
}

