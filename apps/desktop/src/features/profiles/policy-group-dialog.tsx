import { useMemo, useState } from "react";
import { ArrowDown, ArrowUp, Layers } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";
import { Checkbox } from "@voya/ui/components/checkbox";
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
import { SelectField, TextField } from "@voya/ui/components/form-fields";
import { Spinner } from "@voya/ui/components/spinner";
import type { PolicyGroup, ProfileListEntry, Subscription } from "@/ipc/bindings";
import { VirtualScrollList } from "@/components/virtual-scroll-list";
import { savePolicyGroup } from "@/ipc/commands";
import { useDialogSubmit } from "@/lib/use-dialog-submit";

import {
  POLICY_GROUP_STRATEGIES,
  POLICY_GROUP_STRATEGY_HINT_KEYS,
  POLICY_GROUP_STRATEGY_KEYS,
} from "./policy-group-labels";
import { profileMemberName } from "./profile-display";

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

/**
 * Rebuilt for every opening, so a cancelled edit never leaks into the next one.
 * Not mounted while closed: it filters the whole node list, and that list
 * changes on every speedtest frame.
 */
export function PolicyGroupDialog(props: Props) {
  return props.open ? <PolicyGroupEditor key={props.group?.id ?? "new"} {...props} /> : null;
}

function PolicyGroupEditor({ group, nodes, onOpenChange, open, subscriptions }: Props) {
  const { t } = useI18n();
  const [form, setForm] = useState<PolicyGroup>(() => group ?? blankGroup());
  const [filter, setFilter] = useState("");
  const { error, fieldErrors, pending, submit } = useDialogSubmit(t);
  const needle = filter.trim().toLowerCase();
  const visibleNodes = useMemo(
    () => nodes.filter((entry) => !needle || entry.profile.remarks.toLowerCase().includes(needle)),
    [needle, nodes],
  );
  const memberIds = useMemo(() => new Set(form.memberIds), [form.memberIds]);
  const remarksById = useMemo(
    () => new Map(nodes.map((entry) => [entry.profile.id, entry.profile.remarks])),
    [nodes],
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

  function moveMember(index: number, step: -1 | 1) {
    setForm((current) => {
      const target = index + step;
      if (target < 0 || target >= current.memberIds.length) return current;
      const ids = [...current.memberIds];
      [ids[index], ids[target]] = [ids[target]!, ids[index]!];
      return { ...current, memberIds: ids };
    });
  }

  async function save() {
    if (!canSave || pending) return;
    await submit(async () => {
      await savePolicyGroup({
        ...form,
        name: form.name.trim(),
        testUrl: form.testUrl?.trim() || null,
      });
      onOpenChange(false);
    });
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !pending && onOpenChange(next)}>
      <ScrollableDialogContent
        height="viewport"
        width="xl"
        closeLabel={t("actions.close")}
        showCloseButton={!pending}
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Layers aria-hidden="true" className="size-4" />
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
            <VirtualScrollList
              aria-label={t("policyGroups.members")}
              className="max-h-56 rounded-md border"
              estimateSize={30}
              itemKey={(entry) => entry.profile.id}
              items={visibleNodes}
              renderItem={(entry) => {
                const id = entry.profile.id;
                return (
                  <label className="flex items-center gap-2 rounded px-2 py-1 text-sm">
                    <Checkbox
                      checked={memberIds.has(id)}
                      onCheckedChange={(checked) => toggleMember(id, checked === true)}
                    />
                    <span className="truncate">{profileMemberName(entry.profile.remarks, id)}</span>
                  </label>
                );
              }}
              role="group"
            />
            {fieldErrors.memberIds ? (
              <p className="text-sm text-danger" role="alert">
                {fieldErrors.memberIds}
              </p>
            ) : null}
          </div>
          {/* Pick order is failover priority; show it and let it change. */}
          {form.strategy === "fallback" && form.memberIds.length > 1 ? (
            <div className="grid gap-1">
              <span className="text-sm font-medium">{t("policyGroups.order")}</span>
              <span className="text-xs text-muted-foreground">{t("policyGroups.orderHint")}</span>
              <ol aria-label={t("policyGroups.order")} className="grid gap-1 rounded-md border p-2">
                {form.memberIds.map((id, index) => {
                  const remarks = remarksById.get(id) ?? "";
                  const name = profileMemberName(remarks, id);
                  return (
                    <li className="flex items-center gap-2 text-sm" key={id}>
                      <span className="w-5 text-end text-xs tabular-nums text-muted-foreground">
                        {index + 1}
                      </span>
                      <span className="min-w-0 flex-1 truncate">{name}</span>
                      <Button
                        aria-label={t("policyGroups.moveUp", { name })}
                        disabled={index === 0}
                        onClick={() => moveMember(index, -1)}
                        size="icon-sm"
                        type="button"
                        variant="ghost"
                      >
                        <ArrowUp aria-hidden="true" className="size-3.5" />
                      </Button>
                      <Button
                        aria-label={t("policyGroups.moveDown", { name })}
                        disabled={index === form.memberIds.length - 1}
                        onClick={() => moveMember(index, 1)}
                        size="icon-sm"
                        type="button"
                        variant="ghost"
                      >
                        <ArrowDown aria-hidden="true" className="size-3.5" />
                      </Button>
                    </li>
                  );
                })}
              </ol>
            </div>
          ) : null}
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
          <Button disabled={pending} onClick={() => onOpenChange(false)} type="button" variant="outline">
            {t("actions.cancel")}
          </Button>
          <Button disabled={!canSave || pending} onClick={() => void save()} type="button">
            {pending ? <Spinner className="size-4" /> : null}
            {t("actions.save")}
          </Button>
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}
