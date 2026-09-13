import { useMemo, useState } from "react";
import { Layers, LoaderCircle } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";
import { Checkbox } from "@voya/ui/components/checkbox";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@voya/ui/components/dialog";
import { Disclosure } from "@voya/ui/components/disclosure";
import { SelectField, TextField } from "@voya/ui/components/form-fields";
import { getErrorMessage } from "@voya/utils/error";
import type { PolicyGroup, ProfileListEntry, Subscription } from "@/ipc/bindings";
import { IpcCommandError, savePolicyGroup } from "@/ipc/commands";
import { validationFieldErrors } from "@/ipc/messages";

import {
  POLICY_GROUP_STRATEGIES,
  POLICY_GROUP_STRATEGY_HINT_KEYS,
  POLICY_GROUP_STRATEGY_KEYS,
} from "./policy-group-labels";
import { profileNameWithoutFlag } from "./profile-display";

type Props = {
  group: PolicyGroup | null;
  nodes: readonly ProfileListEntry[];
  onOpenChange: (open: boolean) => void;
  open: boolean;
  subscriptions: readonly Subscription[];
};

function blankGroup(): PolicyGroup {
  return {
    autoCreated: false,
    id: "",
    intervalSeconds: null,
    memberIds: [],
    name: "",
    selectedProfileId: null,
    sourceSubscriptionId: null,
    strategy: "urlTest",
    testUrl: null,
    toleranceMs: null,
  };
}

/** An empty field means "use the default"; anything unreadable is dropped too. */
function optionalCount(value: string) {
  const parsed = Number(value.trim());
  return value.trim() && Number.isFinite(parsed) ? Math.max(0, Math.trunc(parsed)) : null;
}

/** Rebuilt for every opening, so a cancelled edit never leaks into the next one. */
export function PolicyGroupDialog(props: Props) {
  return <PolicyGroupEditor key={`${props.group?.id ?? "new"}:${props.open}`} {...props} />;
}

function PolicyGroupEditor({ group, nodes, onOpenChange, open, subscriptions }: Props) {
  const { t } = useI18n();
  const [form, setForm] = useState<PolicyGroup>(() => group ?? blankGroup());
  const [filter, setFilter] = useState("");
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const needle = filter.trim().toLowerCase();
  const visibleNodes = useMemo(
    () => nodes.filter((entry) => !needle || entry.profile.remarks.toLowerCase().includes(needle)),
    [needle, nodes],
  );
  const canSave =
    form.name.trim() !== "" && (form.memberIds.length > 0 || form.sourceSubscriptionId !== null);

  function patch(next: Partial<PolicyGroup>) {
    setForm((current) => ({ ...current, ...next }));
  }

  /** Members keep the order they were picked in; that order is the fallback priority. */
  function toggleMember(id: string, selected: boolean) {
    setForm((current) => {
      const rest = current.memberIds.filter((item) => item !== id);
      return { ...current, memberIds: selected ? [...rest, id] : rest };
    });
  }

  async function submit() {
    if (!canSave || pending) return;
    setPending(true);
    setError(null);
    setFieldErrors({});
    try {
      await savePolicyGroup({
        ...form,
        name: form.name.trim(),
        testUrl: form.testUrl?.trim() || null,
      });
      onOpenChange(false);
    } catch (cause) {
      if (cause instanceof IpcCommandError && cause.appError.kind.type === "validation") {
        setFieldErrors(validationFieldErrors(t, cause.appError.kind.issues));
      } else {
        setError(getErrorMessage(cause));
      }
    } finally {
      setPending(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !pending && onOpenChange(next)}>
      <DialogContent
        className="max-h-[calc(100dvh-2rem)] grid-rows-[auto_minmax(0,1fr)_auto] sm:max-w-xl"
        closeLabel={t("actions.close")}
        showCloseButton={!pending}
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Layers aria-hidden="true" className="size-5" />
            {t(form.id ? "policyGroups.edit" : "policyGroups.new")}
          </DialogTitle>
          <DialogDescription>{t(POLICY_GROUP_STRATEGY_HINT_KEYS[form.strategy])}</DialogDescription>
        </DialogHeader>
        <DialogBody className="grid gap-4 overflow-y-auto">
          <TextField
            error={fieldErrors.name}
            id="policy-group-name"
            label={t("policyGroups.name")}
            onChange={(name) => patch({ name })}
            value={form.name}
          />
          <SelectField
            id="policy-group-strategy"
            label={t("policyGroups.strategyLabel")}
            onChange={(strategy) => patch({ strategy: strategy as PolicyGroup["strategy"] })}
            options={POLICY_GROUP_STRATEGIES.map((value) => ({
              label: t(POLICY_GROUP_STRATEGY_KEYS[value]),
              value,
            }))}
            value={form.strategy}
          />
          <SelectField
            description={t("policyGroups.subscriptionHint")}
            id="policy-group-subscription"
            label={t("policyGroups.subscription")}
            onChange={(id) => patch({ sourceSubscriptionId: id || null })}
            options={[
              { label: t("policyGroups.subscriptionNone"), value: "" },
              ...subscriptions.map((item) => ({ label: item.remarks || item.id, value: item.id })),
            ]}
            value={form.sourceSubscriptionId ?? ""}
          />
          <div className="grid gap-2">
            <div className="flex items-center justify-between gap-2">
              <span className="text-sm font-medium">{t("policyGroups.members")}</span>
              <span className="text-xs text-muted-foreground">
                {t("policyGroups.membersSelected", { count: form.memberIds.length })}
              </span>
            </div>
            <TextField
              id="policy-group-filter"
              label={t("policyGroups.filterNodes")}
              onChange={setFilter}
              value={filter}
            />
            <div
              aria-label={t("policyGroups.members")}
              className="grid max-h-56 gap-1 overflow-y-auto rounded-md border p-2"
              role="group"
            >
              {visibleNodes.map((entry) => {
                const id = entry.profile.id;
                return (
                  <label className="flex items-center gap-2 rounded px-2 py-1 text-sm" key={id}>
                    <Checkbox
                      checked={form.memberIds.includes(id)}
                      onCheckedChange={(checked) => toggleMember(id, checked === true)}
                    />
                    <span className="truncate">{profileNameWithoutFlag(entry.profile.remarks) || id}</span>
                  </label>
                );
              })}
            </div>
            {fieldErrors.memberIds ? (
              <p className="text-sm text-danger" role="alert">
                {fieldErrors.memberIds}
              </p>
            ) : null}
          </div>
          {form.strategy === "selector" ? null : (
            <Disclosure
              invalid={Boolean(
                fieldErrors.testUrl || fieldErrors.intervalSeconds || fieldErrors.toleranceMs,
              )}
              title={t("common.advanced")}
            >
              <div className="grid gap-3">
                <TextField
                  error={fieldErrors.testUrl}
                  id="policy-group-test-url"
                  label={t("policyGroups.testUrl")}
                  onChange={(testUrl) => patch({ testUrl: testUrl || null })}
                  value={form.testUrl ?? ""}
                />
                <TextField
                  error={fieldErrors.intervalSeconds}
                  id="policy-group-interval"
                  inputMode="numeric"
                  label={t("policyGroups.interval")}
                  onChange={(value) => patch({ intervalSeconds: optionalCount(value) })}
                  value={form.intervalSeconds?.toString() ?? ""}
                />
                {form.strategy === "urlTest" ? (
                  <TextField
                    error={fieldErrors.toleranceMs}
                    id="policy-group-tolerance"
                    inputMode="numeric"
                    label={t("policyGroups.tolerance")}
                    onChange={(value) => patch({ toleranceMs: optionalCount(value) })}
                    value={form.toleranceMs?.toString() ?? ""}
                  />
                ) : null}
              </div>
            </Disclosure>
          )}
          {error ? (
            <p className="text-sm text-danger" role="alert">
              {error}
            </p>
          ) : null}
        </DialogBody>
        <DialogFooter>
          <Button disabled={pending} onClick={() => onOpenChange(false)} type="button" variant="ghost">
            {t("actions.cancel")}
          </Button>
          <Button disabled={!canSave || pending} onClick={() => void submit()} type="button">
            {pending ? <LoaderCircle aria-hidden="true" className="size-4 animate-spin" /> : null}
            {t("actions.save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
