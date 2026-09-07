import { useState } from "react";
import type * as React from "react";
import { Route, Save } from "lucide-react";
import { useI18n } from "@voya/i18n/use-i18n";

import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import type { RoutingRule, RoutingRuleScope } from "@/ipc/bindings";
import { translateFieldErrors, zodIssuesToErrorMap, type FieldErrorMap } from "@/lib/zod-errors";

import { RULE_TYPES } from "./routing-constants";
import { CheckboxField, SelectField, TextAreaField, TextField } from "./routing-form-fields";
import {
  routingRuleSchema,
  type RoutingRulePayload,
} from "./routing-form-schema";
import { formToRule, ruleToForm } from "./routing-form-values";

export function RoutingRuleDialog({
  mode,
  onOpenChange,
  onSubmit,
  open,
  rule,
}: {
  mode: "create" | "edit";
  onOpenChange: (open: boolean) => void;
  onSubmit: (rule: RoutingRulePayload) => Promise<void>;
  open: boolean;
  rule: RoutingRule | null;
}) {
  const { t } = useI18n();
  const [form, setForm] = useState(() => ruleToForm(rule));
  const [fieldErrors, setFieldErrors] = useState<FieldErrorMap>({});
  const errors = translateFieldErrors(t, fieldErrors);

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

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <ScrollableDialogContent closeLabel={t("actions.close")} width="56rem">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Route className="size-4" aria-hidden="true" />
            {t(mode === "edit" ? "panes.routing.editRule" : "panes.routing.createRule")}
          </DialogTitle>
          <DialogDescription className="sr-only">{t("panes.routing.ruleEditor")}</DialogDescription>
        </DialogHeader>

        <form
          className="grid min-h-0 gap-4 overflow-y-auto pe-1"
          id="routing-rule-form"
          onSubmit={(event) => void submitForm(event)}
        >
          <div className="grid gap-3 sm:grid-cols-[1fr_10rem_10rem]">
            <TextField
              error={errors.remarks}
              label={t("panes.routing.remarks")}
              onChange={(value) => setForm((current) => ({ ...current, remarks: value }))}
              value={form.remarks}
            />
            <SelectField
              error={errors.scope}
              label={t("panes.routing.ruleScope")}
              onChange={(value) => setForm((current) => ({ ...current, scope: value as RoutingRuleScope }))}
              options={[
                { label: t("panes.routing.scopeAll"), value: String(RULE_TYPES.All) },
                { label: t("panes.routing.scopeRouting"), value: String(RULE_TYPES.Routing) },
                { label: "DNS", value: String(RULE_TYPES.Dns) },
              ]}
              value={String(form.scope)}
            />
            <TextField
              error={errors.outbound}
              label={t("panes.routing.outbound")}
              onChange={(value) => setForm((current) => ({ ...current, outbound: value }))}
              value={form.outbound}
            />
          </div>
          <div className="grid gap-3 sm:grid-cols-3">
            <TextField
              error={errors.port}
              label={t("panes.routing.port")}
              onChange={(value) => setForm((current) => ({ ...current, port: value }))}
              value={form.port}
            />
            <TextField
              error={errors.network}
              label={t("panes.routing.network")}
              onChange={(value) => setForm((current) => ({ ...current, network: value }))}
              value={form.network}
            />
            <TextField
              error={errors.kind}
              label={t("panes.routing.type")}
              onChange={(value) => setForm((current) => ({ ...current, kind: value }))}
              value={form.kind}
            />
          </div>
          <div className="grid gap-3 lg:grid-cols-2">
            <TextAreaField
              error={errors.domain}
              label={t("panes.routing.domain")}
              onChange={(value) => setForm((current) => ({ ...current, domain: value }))}
              value={form.domain}
            />
            <TextAreaField
              error={errors.ip}
              label="IP"
              onChange={(value) => setForm((current) => ({ ...current, ip: value }))}
              value={form.ip}
            />
            <TextAreaField
              error={errors.protocol}
              label={t("panes.routing.protocol")}
              onChange={(value) => setForm((current) => ({ ...current, protocol: value }))}
              value={form.protocol}
            />
            <TextAreaField
              error={errors.process}
              label={t("panes.routing.process")}
              onChange={(value) => setForm((current) => ({ ...current, process: value }))}
              value={form.process}
            />
            <TextAreaField
              error={errors.inboundTags}
              label={t("panes.routing.inboundTags")}
              onChange={(value) => setForm((current) => ({ ...current, inboundTags: value }))}
              value={form.inboundTags}
            />
          </div>
          <CheckboxField
            checked={form.enabled}
            label={t("panes.routing.enabled")}
            onCheckedChange={(checked) => setForm((current) => ({ ...current, enabled: checked }))}
          />
        </form>

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
