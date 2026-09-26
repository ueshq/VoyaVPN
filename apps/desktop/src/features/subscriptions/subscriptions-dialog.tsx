import { useId, useRef, useState } from "react";
import { Rss } from "lucide-react";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
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
import { SwitchField, TextField } from "@voya/ui/components/form-fields";
import { Spinner } from "@voya/ui/components/spinner";
import { voyaCommands } from "@voya/client/transport";
import type { Subscription } from "@voya/contracts";
import { useDialogSubmit } from "@/lib/use-dialog-submit";
import { assertSubscriptionUpdated } from "@voya/features/subscriptions/subscription-update-result";
import { subscriptionFormSchema } from "@voya/features/subscriptions/subscriptions-form-schema";

/** Fields the dialog validates; also the suffixes of the input ids. */
type SubscriptionField = "hours" | "remarks" | "url";

type Props = {
  subscription?: Subscription | null;
  onCloseFocus?: () => void;
  onOpenChange: (open: boolean) => void;
  open: boolean;
};

function blankSubscription(): Subscription {
  return {
    id: "",
    remarks: "",
    url: "",
    additionalUrl: "",
    userAgent: "",
    enabled: false,
    sort: 0,
    filter: null,
    converterTarget: null,
    autoUpdateIntervalMinutes: null,
  };
}

export function SubscriptionsDialog(props: Props) {
  return (
    <SubscriptionEditor
      key={`${props.subscription?.id ?? "new"}:${props.open}`}
      {...props}
    />
  );
}

function SubscriptionEditor({
  subscription,
  onCloseFocus,
  onOpenChange,
  open,
}: Props) {
  const { t } = useI18n();
  const id = useId();
  const formRef = useRef<HTMLFormElement>(null);
  const [touched, setTouched] = useState<ReadonlySet<string>>(() => new Set());
  const [submitted, setSubmitted] = useState(false);
  const [form, setForm] = useState<Subscription>(
    () => subscription
      ? { ...subscription, enabled: subscription.enabled && (subscription.autoUpdateIntervalMinutes ?? 0) > 0 }
      : blankSubscription(),
  );
  const [hours, setHours] = useState(() => {
    const minutes = subscription?.autoUpdateIntervalMinutes ?? 0;
    return subscription?.enabled && minutes > 0 ? String(minutes / 60) : "1";
  });
  const [needsUpdate, setNeedsUpdate] = useState(false);
  const {
    error,
    fieldErrors: serverFieldErrors,
    pending,
    submit,
  } = useDialogSubmit(t);

  const parsed = subscriptionFormSchema.safeParse({
    enabled: form.enabled,
    hours,
    remarks: form.remarks,
    url: form.url,
  });
  const schemaErrors: Partial<Record<SubscriptionField, TranslationKey>> = parsed.success
    ? {}
    : Object.fromEntries(
        parsed.error.issues.map((issue) => [issue.path[0], issue.message]),
      );
  function fieldFeedback(field: SubscriptionField) {
    // Backend validation issues arrive pre-translated; local schema issues are keys.
    const serverError = serverFieldErrors[field];
    if (serverError) return serverError;
    // Local checks stay quiet until the field was touched or the form was sent.
    const schemaError = (submitted || touched.has(field)) && schemaErrors[field];
    return schemaError ? t(schemaError) : undefined;
  }
  function touch(field: string) {
    setTouched((current) => new Set(current).add(field));
  }
  function changeOpen(next: boolean) {
    if (!pending) onOpenChange(next);
  }
  async function submitForm() {
    setSubmitted(true);
    if (!parsed.success) {
      const invalid = parsed.error.issues[0]?.path[0];
      if (typeof invalid === "string") {
        formRef.current
          ?.querySelector<HTMLInputElement>(`[id="${id}-${invalid}"]`)
          ?.focus();
      }
      return;
    }
    await submit(async () => {
      const create = !form.id;
      const saved = await voyaCommands().saveSubscription({
        ...form,
        remarks: form.remarks.trim(),
        url: form.url.trim(),
        autoUpdateIntervalMinutes:
          form.enabled
            ? Math.max(1, Math.round(Number(hours) * 60))
            : null,
      });
      setForm(saved);
      if (create || needsUpdate) {
        setNeedsUpdate(true);
        const result = await voyaCommands().updateSubscriptions(saved.id, true, null);
        assertSubscriptionUpdated(result, t);
        setNeedsUpdate(false);
      }
      onOpenChange(false);
    });
  }
  return (
    <Dialog open={open} onOpenChange={changeOpen}>
      <ScrollableDialogContent
        height="viewport"
        width="xl"
        closeLabel={t("actions.close")}
        showCloseButton={!pending}
        onCloseAutoFocus={
          onCloseFocus
            ? (event) => {
                event.preventDefault();
                onCloseFocus();
              }
            : undefined
        }
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Rss aria-hidden="true" className="size-4" />
            {t(
              subscription ? "subscriptions.edit" : "home.subscriptionCard.add",
            )}
          </DialogTitle>
          <DialogDescription>{t("subscriptions.sourceHint")}</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <form
            ref={formRef}
            id="subscription-form"
            className="grid gap-4"
            onSubmit={(event) => {
              event.preventDefault();
              void submitForm();
            }}
          >
            <TextField
              id={`${id}-remarks`}
              required
              description={t("subscriptions.required")}
              error={fieldFeedback("remarks")}
              onBlur={() => touch("remarks")}
              disabled={pending}
              label={t("panes.subscriptions.remarks")}
              value={form.remarks}
              onChange={(remarks) =>
                setForm((current) => ({ ...current, remarks }))
              }
            />
            <TextField
              id={`${id}-url`}
              required
              description={t("subscriptions.required")}
              error={fieldFeedback("url")}
              onBlur={() => touch("url")}
              disabled={pending}
              label={t("panes.subscriptions.url")}
              value={form.url}
              onChange={(url) => setForm((current) => ({ ...current, url }))}
            />
            <SwitchField
              disabled={pending}
              checked={form.enabled}
              label={t("panes.subscriptions.enabled")}
              onChange={(enabled) => setForm((current) => ({ ...current, enabled }))}
            />
            {form.enabled ? <TextField
              id={`${id}-hours`}
              inputClassName="w-36"
              required
              disabled={pending}
              label={t("panes.subscriptions.autoUpdateInterval")}
              description={t("panes.subscriptions.autoUpdateHint")}
              error={fieldFeedback("hours")}
              onBlur={() => touch("hours")}
              value={hours}
              onChange={setHours}
            /> : null}
            <Disclosure title={t("common.advanced")}>
              <TextField
                disabled={pending}
                label={t("panes.subscriptions.userAgent")}
                value={form.userAgent}
                onChange={(userAgent) =>
                  setForm((current) => ({ ...current, userAgent }))
                }
              />
              <TextField
                disabled={pending}
                label={t("panes.subscriptions.additionalUrl")}
                value={form.additionalUrl}
                onChange={(additionalUrl) =>
                  setForm((current) => ({ ...current, additionalUrl }))
                }
              />
              <TextField
                disabled={pending}
                label={t("panes.subscriptions.filter")}
                value={form.filter ?? ""}
                onChange={(filter) =>
                  setForm((current) => ({ ...current, filter: filter || null }))
                }
              />
              <TextField
                disabled={pending}
                label={t("panes.subscriptions.convertTarget")}
                value={form.converterTarget ?? ""}
                onChange={(converterTarget) =>
                  setForm((current) => ({
                    ...current,
                    converterTarget: converterTarget || null,
                  }))
                }
              />
            </Disclosure>
            {error ? (
              <Alert variant="destructive">
                <AlertDescription>
                  {needsUpdate ? (
                    <p>{t("subscriptions.savedUpdateFailed")}</p>
                  ) : null}
                  <p className="whitespace-pre-wrap break-words">{error}</p>
                </AlertDescription>
              </Alert>
            ) : null}
          </form>
        </DialogBody>
        <DialogFooter>
          <Button
            variant="outline"
            disabled={pending}
            onClick={() => changeOpen(false)}
          >
            {t("actions.cancel")}
          </Button>
          <Button
            form="subscription-form"
            type="submit"
            disabled={pending}
          >
            {pending ? (
              <Spinner className="size-4" />
            ) : null}
            {needsUpdate
              ? t("subscriptions.retryUpdate")
              : subscription
                ? t("actions.save")
                : t("subscriptions.addAndUpdate")}
          </Button>
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}
