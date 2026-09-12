import { useState } from "react";
import type * as React from "react";
import { Route, Save } from "lucide-react";

import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogBody,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@voya/ui/components/dialog";
import type { Routing_Serialize } from "@/ipc/bindings";
import { useI18n } from "@voya/i18n/use-i18n";
import {
  translateFieldErrors,
  zodIssuesToErrorMap,
  type FieldErrorMap,
} from "@/lib/zod-errors";

import { SINGBOX_DOMAIN_STRATEGIES } from "./routing-constants";
import {
  CheckboxField,
  SelectField,
  TextField,
} from "@voya/ui/components/form-fields";
import {
  routingProfileFieldsSchema,
  type RoutingFormPayload,
} from "./routing-form-schema";
import { routingToForm } from "./routing-form-values";

/** Fields this dialog renders an inline error for; anything else needs the form-level alert. */
const RENDERED_FIELDS = new Set([
  "remarks",
  "singboxDomainStrategy",
  "singboxRulesetPath",
]);

export function RoutingProfileDialog({
  mode,
  onOpenChange,
  onSubmit,
  open,
  routing,
}: {
  mode: "create" | "edit";
  onOpenChange: (open: boolean) => void;
  onSubmit: (routing: RoutingFormPayload) => Promise<void>;
  open: boolean;
  routing: Routing_Serialize | null;
}) {
  const { t } = useI18n();
  const [form, setForm] = useState(() => routingToForm(routing));
  const [fieldErrors, setFieldErrors] = useState<FieldErrorMap>({});
  const errors = translateFieldErrors(t, fieldErrors);
  // Nothing here edits a rule, but an issue keyed `rules.3.port` (or any other
  // field this dialog does not render) would otherwise make Save a silent no-op.
  const formError = Object.entries(errors).find(
    ([field]) => !RENDERED_FIELDS.has(field),
  )?.[1];

  async function submitForm(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    // The rule set is carried through untouched: validating rules the user
    // cannot see here would reject profiles whose stored rules predate a
    // stricter validator, with no field to attach the message to.
    const { rules, ...profileFields } = form;
    const parsed = routingProfileFieldsSchema.safeParse(profileFields);
    if (!parsed.success) {
      setFieldErrors(zodIssuesToErrorMap(parsed.error));
      return;
    }
    setFieldErrors({});
    await onSubmit({ ...parsed.data, rules });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="w-[min(96vw,42rem)] max-h-[calc(100dvh-2rem)] grid-rows-[auto_minmax(0,1fr)_auto]"
        closeLabel={t("actions.close")}
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Route className="size-4" aria-hidden="true" />
            {t(
              mode === "edit"
                ? "panes.routing.editProfile"
                : "panes.routing.createProfile",
            )}
          </DialogTitle>
          <DialogDescription className="sr-only">
            {t("panes.routing.editor")}
          </DialogDescription>
        </DialogHeader>
        <DialogBody>
          <form
            className="grid gap-4"
            id="routing-profile-form"
            onSubmit={(event) => void submitForm(event)}
          >
            <TextField
              error={errors.remarks}
              label={t("panes.routing.remarks")}
              onChange={(value) =>
                setForm((current) => ({ ...current, remarks: value }))
              }
              value={form.remarks}
            />
            <div className="grid gap-3">
              <SelectField
                error={errors.singboxDomainStrategy}
                label={t("panes.routing.domainStrategy")}
                onChange={(value) =>
                  setForm((current) => ({
                    ...current,
                    singboxDomainStrategy: value,
                  }))
                }
                options={SINGBOX_DOMAIN_STRATEGIES.map((strategy) => ({
                  label: strategy || t("panes.routing.defaultValue"),
                  value: strategy,
                }))}
                value={form.singboxDomainStrategy}
              />
            </div>
            <TextField
              error={errors.singboxRulesetPath}
              label={t("panes.routing.rulesetPath")}
              onChange={(value) =>
                setForm((current) => ({
                  ...current,
                  singboxRulesetPath: value,
                }))
              }
              value={form.singboxRulesetPath}
            />
            <CheckboxField
              checked={form.enabled}
              label={t("panes.routing.enabled")}
              onChange={(checked) =>
                setForm((current) => ({ ...current, enabled: checked }))
              }
            />
            {formError ? (
              <Alert variant="destructive">
                <AlertDescription>{formError}</AlertDescription>
              </Alert>
            ) : null}
          </form>
        </DialogBody>
        <DialogFooter>
          <Button
            onClick={() => onOpenChange(false)}
            type="button"
            variant="outline"
          >
            {t("actions.cancel")}
          </Button>
          <Button form="routing-profile-form" type="submit">
            <Save className="size-4" aria-hidden="true" />
            {t("actions.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
