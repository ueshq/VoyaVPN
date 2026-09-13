import { useId, useState } from "react";
import type * as React from "react";
import { Route, Save } from "lucide-react";

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
import {
  translateFieldErrors,
  zodIssuesToErrorMap,
  type FieldErrorMap,
} from "@/lib/zod-errors";

import { OUTBOUND_LABEL_KEYS } from "./rule-outbound";
import { RULE_SCOPE_LABEL_KEYS } from "./routing-constants";
import { routingRuleSchema, type RoutingRulePayload } from "./routing-form-schema";
import { formToRule, ruleToForm, type RuleFormState } from "./routing-form-values";
import { sentinelLabelKey } from "./sentinel-rules";

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
  rule,
}: {
  mode: "create" | "edit";
  /** Node remarks a rule can target; `null` while the node list loads. */
  nodeNames: readonly string[] | null;
  onOpenChange: (open: boolean) => void;
  onSubmit: (rule: RoutingRulePayload) => Promise<void>;
  open: boolean;
  rule: RoutingRule | null;
}) {
  const { t } = useI18n();
  const networkId = useId();
  const [form, setForm] = useState(() => ruleToForm(rule));
  const [fieldErrors, setFieldErrors] = useState<FieldErrorMap>({});
  const errors = translateFieldErrors(t, fieldErrors);
  const formError = Object.entries(errors).find(([field]) => !RENDERED_FIELDS.has(field))?.[1];
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
    ...(nodeNames ?? []).map((name) => ({ label: name, value: name })),
  ];
  if (!outboundOptions.some((option) => option.value === form.outbound)) {
    // A node the rule names stays selectable even when it no longer exists, so
    // opening the editor never retargets the rule behind the user's back.
    outboundOptions.push({
      label:
        nodeNames === null
          ? form.outbound
          : t("panes.routing.outboundMissing", { name: form.outbound }),
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
            <TextAreaField
              description={t("panes.routing.domainHelp")}
              error={errors.domain}
              label={t("panes.routing.domain")}
              onChange={(value) => update("domain", value)}
              value={form.domain}
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
            <TextAreaField
              description={t("panes.routing.processHelp")}
              error={errors.process}
              label={t("panes.routing.process")}
              onChange={(value) => update("process", value)}
              value={form.process}
            />
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
            <Save className="size-4" aria-hidden="true" />
            {t("actions.save")}
          </Button>
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}
