import { useMemo, useState } from "react";
import type * as React from "react";
import {
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  FilePlus2,
  Globe2,
  Pencil,
  Play,
  Plus,
  Route,
  Save,
  Trash2,
} from "lucide-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  deleteRoutingRules,
  deleteRoutings,
  importRoutingTemplates,
  listRoutings,
  loadAppConfig,
  moveRoutingRule,
  saveAppConfig,
  saveRouting,
  saveRoutingRule,
  setActiveRouting,
} from "@/ipc";
import type {
  AppConfig_Serialize,
  RoutingItem_Deserialize,
  RoutingItem_Serialize,
  RulesItem_Deserialize,
  RulesItem_Serialize,
} from "@/ipc/bindings";
import { cn } from "@/lib/utils";

const MOVE_ACTIONS = {
  Top: 1,
  Up: 2,
  Down: 3,
  Bottom: 4,
  Position: 5,
} as const;

const RULE_TYPES = {
  All: 0,
  Routing: 1,
  Dns: 2,
} as const;

const DOMAIN_STRATEGIES = ["AsIs", "IPIfNonMatch", "IPOnDemand"];
const SINGBOX_DOMAIN_STRATEGIES = ["", "prefer_ipv4", "prefer_ipv6", "ipv4_only", "ipv6_only"];

type RoutingDialogState =
  | { mode: "create"; routing?: null }
  | { mode: "edit"; routing: RoutingItem_Serialize }
  | null;

type RuleDialogState =
  | { mode: "create"; rule?: null }
  | { mode: "edit"; rule: RulesItem_Serialize }
  | null;

export function RoutingScreen() {
  const queryClient = useQueryClient();
  const [operationError, setOperationError] = useState<string | null>(null);
  const [routingDialog, setRoutingDialog] = useState<RoutingDialogState>(null);
  const [ruleDialog, setRuleDialog] = useState<RuleDialogState>(null);
  const [selectedRoutingId, setSelectedRoutingId] = useState<string | null>(null);
  const [selectedRuleId, setSelectedRuleId] = useState<string | null>(null);
  const [templateUrlDraft, setTemplateUrlDraft] = useState<string | null>(null);
  const routingsQuery = useQuery({
    queryFn: listRoutings,
    queryKey: ["routings"],
  });
  const configQuery = useQuery({
    queryFn: loadAppConfig,
    queryKey: ["app-config"],
  });
  const routings = useMemo(() => routingsQuery.data ?? [], [routingsQuery.data]);
  const selectedRouting = useMemo(
    () =>
      routings.find((routing) => routing.Id === selectedRoutingId) ??
      routings.find((routing) => routing.IsActive) ??
      routings[0] ??
      null,
    [routings, selectedRoutingId],
  );
  const selectedRule =
    selectedRouting?.RuleSet.find((rule) => rule.Id === selectedRuleId) ??
    selectedRouting?.RuleSet[0] ??
    null;
  const templateUrl = templateUrlDraft ?? configQuery.data?.ConstItem.RouteRulesTemplateSourceUrl ?? "";

  async function runOperation(operation: () => Promise<unknown>) {
    setOperationError(null);
    try {
      await operation();
      await queryClient.invalidateQueries({ queryKey: ["routings"] });
    } catch (error) {
      setOperationError(error instanceof Error ? error.message : String(error));
    }
  }

  async function saveTemplateUrl(config: AppConfig_Serialize | undefined, url: string) {
    const current = config ?? (await loadAppConfig());
    await saveAppConfig({
      ...current,
      ConstItem: {
        ...current.ConstItem,
        RouteRulesTemplateSourceUrl: url.trim() || null,
      },
    });
    setTemplateUrlDraft(url);
    await queryClient.invalidateQueries({ queryKey: ["app-config"] });
  }

  async function handleImportTemplates() {
    await runOperation(async () => {
      await saveTemplateUrl(configQuery.data, templateUrl);
      await importRoutingTemplates(false, null, false);
    });
  }

  async function handleSaveRouting(routing: RoutingFormState) {
    await runOperation(async () => {
      const saved = await saveRouting(routing as RoutingItem_Deserialize);
      setSelectedRoutingId(saved.Id);
    });
    setRoutingDialog(null);
  }

  async function handleSaveRule(rule: RulePayload) {
    if (!selectedRouting) {
      return;
    }
    await runOperation(async () => {
      const saved = await saveRoutingRule(selectedRouting.Id, rule as RulesItem_Deserialize);
      setSelectedRoutingId(saved.Id);
      setSelectedRuleId(rule.Id ?? saved.RuleSet.at(-1)?.Id ?? null);
    });
    setRuleDialog(null);
  }

  return (
    <section className="flex h-full min-h-0 flex-col" aria-label="Routing">
      <div className="flex min-h-14 shrink-0 flex-wrap items-center gap-2 border-b px-4 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <Route className="size-4 text-muted-foreground" aria-hidden="true" />
          <h2 className="text-sm font-semibold">Routing</h2>
          <span className="rounded-md border px-2 py-1 text-xs text-muted-foreground">
            {routings.length.toLocaleString()} profiles
          </span>
        </div>

        <label className="ms-auto flex h-9 min-w-[18rem] max-w-xl flex-1 items-center gap-2 rounded-md border bg-card px-3 text-sm md:flex-none">
          <Globe2 className="size-4 text-muted-foreground" aria-hidden="true" />
          <span className="sr-only">Template URL</span>
          <input
            className="min-w-0 flex-1 bg-transparent outline-none placeholder:text-muted-foreground"
            onChange={(event) => setTemplateUrlDraft(event.target.value)}
            placeholder="RouteRulesTemplateSourceUrl"
            value={templateUrl}
          />
        </label>
        <Button onClick={() => void handleImportTemplates()} size="sm" type="button" variant="outline">
          <FilePlus2 className="size-4" aria-hidden="true" />
          Import templates
        </Button>
        <Button onClick={() => setRoutingDialog({ mode: "create" })} size="sm" type="button">
          <Plus className="size-4" aria-hidden="true" />
          Profile
        </Button>
        <Button
          disabled={!selectedRouting}
          onClick={() => selectedRouting && setRoutingDialog({ mode: "edit", routing: selectedRouting })}
          size="sm"
          type="button"
          variant="outline"
        >
          <Pencil className="size-4" aria-hidden="true" />
          Edit
        </Button>
        <Button
          disabled={!selectedRouting || selectedRouting.IsActive}
          onClick={() => selectedRouting && void runOperation(() => setActiveRouting(selectedRouting.Id))}
          size="sm"
          type="button"
          variant="outline"
        >
          <Play className="size-4" aria-hidden="true" />
          Activate
        </Button>
        <Button
          disabled={!selectedRouting}
          onClick={() =>
            selectedRouting &&
            void runOperation(async () => {
              await deleteRoutings([selectedRouting.Id]);
              setSelectedRoutingId(null);
              setSelectedRuleId(null);
            })
          }
          size="sm"
          type="button"
          variant="outline"
        >
          <Trash2 className="size-4" aria-hidden="true" />
          Delete
        </Button>
      </div>

      {operationError ? (
        <div className="border-b bg-destructive/10 px-4 py-2 text-sm text-destructive">{operationError}</div>
      ) : null}

      <div className="grid min-h-0 flex-1 grid-cols-1 lg:grid-cols-[21rem_1fr]">
        <aside className="min-h-0 border-b lg:border-b-0 lg:border-e">
          <div className="h-10 border-b px-4 py-2 text-xs font-medium uppercase text-muted-foreground">
            Profiles
          </div>
          <div className="h-[18rem] overflow-auto lg:h-full">
            {routings.map((routing) => (
              <button
                className={cn(
                  "flex min-h-14 w-full items-center gap-3 border-b px-4 py-2 text-start hover:bg-accent",
                  selectedRouting?.Id === routing.Id ? "bg-accent text-accent-foreground" : "",
                )}
                key={routing.Id}
                onClick={() => {
                  setSelectedRoutingId(routing.Id);
                  setSelectedRuleId(null);
                }}
                type="button"
              >
                <span className="grid size-6 shrink-0 place-items-center rounded-md border bg-background">
                  {routing.IsActive ? <CheckCircle2 className="size-4 text-primary" aria-hidden="true" /> : null}
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm font-medium">{routing.Remarks || "Untitled routing"}</span>
                  <span className="block truncate text-xs text-muted-foreground">
                    {routing.RuleNum} rules · {routing.DomainStrategy || "AsIs"}
                  </span>
                </span>
              </button>
            ))}
          </div>
        </aside>

        <div className="flex min-h-0 flex-col">
          <div className="flex min-h-12 shrink-0 flex-wrap items-center gap-2 border-b px-4 py-2">
            <div className="min-w-0">
              <h3 className="truncate text-sm font-semibold">{selectedRouting?.Remarks ?? "No routing profile"}</h3>
              <p className="truncate text-xs text-muted-foreground">
                {selectedRouting
                  ? `${selectedRouting.RuleNum} rules · Xray ${selectedRouting.DomainStrategy || "AsIs"} · sing-box ${
                      selectedRouting.DomainStrategy4Singbox || "default"
                    }`
                  : ""}
              </p>
            </div>
            <div className="ms-auto flex items-center gap-2">
              <Button disabled={!selectedRouting} onClick={() => setRuleDialog({ mode: "create" })} size="sm" type="button">
                <Plus className="size-4" aria-hidden="true" />
                Rule
              </Button>
              <Button
                disabled={!selectedRule}
                onClick={() => selectedRule && setRuleDialog({ mode: "edit", rule: selectedRule })}
                size="sm"
                type="button"
                variant="outline"
              >
                <Pencil className="size-4" aria-hidden="true" />
                Edit
              </Button>
              <Button
                disabled={!selectedRouting || !selectedRule}
                onClick={() =>
                  selectedRouting &&
                  selectedRule &&
                  void runOperation(() =>
                    moveRoutingRule(selectedRouting.Id, selectedRule.Id, MOVE_ACTIONS.Up, null),
                  )
                }
                size="icon"
                type="button"
                variant="outline"
              >
                <ArrowUp className="size-4" aria-hidden="true" />
              </Button>
              <Button
                disabled={!selectedRouting || !selectedRule}
                onClick={() =>
                  selectedRouting &&
                  selectedRule &&
                  void runOperation(() =>
                    moveRoutingRule(selectedRouting.Id, selectedRule.Id, MOVE_ACTIONS.Down, null),
                  )
                }
                size="icon"
                type="button"
                variant="outline"
              >
                <ArrowDown className="size-4" aria-hidden="true" />
              </Button>
              <Button
                disabled={!selectedRouting || !selectedRule}
                onClick={() =>
                  selectedRouting &&
                  selectedRule &&
                  void runOperation(() => deleteRoutingRules(selectedRouting.Id, [selectedRule.Id]))
                }
                size="sm"
                type="button"
                variant="outline"
              >
                <Trash2 className="size-4" aria-hidden="true" />
                Delete
              </Button>
            </div>
          </div>

          <div className="min-h-0 flex-1 overflow-auto">
            <table className="w-full min-w-[58rem] border-collapse text-sm">
              <thead className="sticky top-0 z-10 bg-background text-xs text-muted-foreground">
                <tr className="border-b">
                  <th className="w-12 px-3 py-2 text-start">#</th>
                  <th className="px-3 py-2 text-start">Remarks</th>
                  <th className="px-3 py-2 text-start">Outbound</th>
                  <th className="px-3 py-2 text-start">Type</th>
                  <th className="px-3 py-2 text-start">Domain</th>
                  <th className="px-3 py-2 text-start">IP</th>
                  <th className="px-3 py-2 text-start">Port</th>
                  <th className="px-3 py-2 text-start">Network</th>
                </tr>
              </thead>
              <tbody>
                {(selectedRouting?.RuleSet ?? []).map((rule, index) => (
                  <tr
                    className={cn(
                      "cursor-default border-b hover:bg-accent/70",
                      selectedRule?.Id === rule.Id ? "bg-accent" : "",
                      !rule.Enabled ? "opacity-55" : "",
                    )}
                    key={rule.Id}
                    onClick={() => setSelectedRuleId(rule.Id)}
                  >
                    <td className="px-3 py-2 tabular-nums text-muted-foreground">{index + 1}</td>
                    <td className="max-w-52 truncate px-3 py-2 font-medium">{rule.Remarks ?? ""}</td>
                    <td className="px-3 py-2">{rule.OutboundTag ?? ""}</td>
                    <td className="px-3 py-2">{formatRuleType(rule.RuleType)}</td>
                    <td className="max-w-72 truncate px-3 py-2">{formatList(rule.Domain)}</td>
                    <td className="max-w-56 truncate px-3 py-2">{formatList(rule.Ip)}</td>
                    <td className="px-3 py-2">{rule.Port ?? ""}</td>
                    <td className="px-3 py-2">{rule.Network ?? ""}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>

      <RoutingProfileDialog
        key={routingDialog?.mode === "edit" ? `routing-${routingDialog.routing.Id}` : `routing-${routingDialog?.mode ?? "closed"}`}
        mode={routingDialog?.mode ?? "create"}
        onOpenChange={(open) => !open && setRoutingDialog(null)}
        onSubmit={handleSaveRouting}
        open={Boolean(routingDialog)}
        routing={routingDialog?.mode === "edit" ? routingDialog.routing : null}
      />
      <RoutingRuleDialog
        key={ruleDialog?.mode === "edit" ? `rule-${ruleDialog.rule.Id}` : `rule-${ruleDialog?.mode ?? "closed"}`}
        mode={ruleDialog?.mode ?? "create"}
        onOpenChange={(open) => !open && setRuleDialog(null)}
        onSubmit={handleSaveRule}
        open={Boolean(ruleDialog)}
        rule={ruleDialog?.mode === "edit" ? ruleDialog.rule : null}
      />
    </section>
  );
}

function RoutingProfileDialog({
  mode,
  onOpenChange,
  onSubmit,
  open,
  routing,
}: {
  mode: "create" | "edit";
  onOpenChange: (open: boolean) => void;
  onSubmit: (routing: RoutingFormState) => Promise<void>;
  open: boolean;
  routing: RoutingItem_Serialize | null;
}) {
  const [form, setForm] = useState<RoutingFormState>(() => routingToForm(routing));

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(96vw,42rem)]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Route className="size-4" aria-hidden="true" />
            {mode === "edit" ? "Edit routing profile" : "Create routing profile"}
          </DialogTitle>
          <DialogDescription className="sr-only">Routing profile editor</DialogDescription>
        </DialogHeader>

        <form
          className="grid gap-4"
          id="routing-profile-form"
          onSubmit={(event) => {
            event.preventDefault();
            void onSubmit(form);
          }}
        >
          <TextField
            label="Remarks"
            onChange={(value) => setForm((current) => ({ ...current, Remarks: value }))}
            value={form.Remarks ?? ""}
          />
          <div className="grid gap-3 sm:grid-cols-2">
            <SelectField
              label="Xray domain strategy"
              onChange={(value) => setForm((current) => ({ ...current, DomainStrategy: value }))}
              value={form.DomainStrategy ?? "AsIs"}
            >
              {DOMAIN_STRATEGIES.map((strategy) => (
                <option key={strategy} value={strategy}>
                  {strategy}
                </option>
              ))}
            </SelectField>
            <SelectField
              label="sing-box domain strategy"
              onChange={(value) => setForm((current) => ({ ...current, DomainStrategy4Singbox: value }))}
              value={form.DomainStrategy4Singbox ?? ""}
            >
              {SINGBOX_DOMAIN_STRATEGIES.map((strategy) => (
                <option key={strategy || "default"} value={strategy}>
                  {strategy || "default"}
                </option>
              ))}
            </SelectField>
          </div>
          <TextField
            label="Ruleset path for sing-box"
            onChange={(value) => setForm((current) => ({ ...current, CustomRulesetPath4Singbox: value }))}
            value={form.CustomRulesetPath4Singbox ?? ""}
          />
          <TextField
            label="Source URL"
            onChange={(value) => setForm((current) => ({ ...current, Url: value }))}
            value={form.Url ?? ""}
          />
          <label className="flex items-center gap-2 text-sm">
            <input
              checked={form.Enabled ?? true}
              onChange={(event) => setForm((current) => ({ ...current, Enabled: event.target.checked }))}
              type="checkbox"
            />
            Enabled
          </label>
        </form>

        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} type="button" variant="outline">
            Cancel
          </Button>
          <Button form="routing-profile-form" type="submit">
            <Save className="size-4" aria-hidden="true" />
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function RoutingRuleDialog({
  mode,
  onOpenChange,
  onSubmit,
  open,
  rule,
}: {
  mode: "create" | "edit";
  onOpenChange: (open: boolean) => void;
  onSubmit: (rule: RulePayload) => Promise<void>;
  open: boolean;
  rule: RulesItem_Serialize | null;
}) {
  const [form, setForm] = useState<RuleFormState>(() => ruleToForm(rule));

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[92vh] w-[min(96vw,56rem)] grid-rows-[auto,minmax(0,1fr),auto] overflow-hidden">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Route className="size-4" aria-hidden="true" />
            {mode === "edit" ? "Edit routing rule" : "Create routing rule"}
          </DialogTitle>
          <DialogDescription className="sr-only">Routing rule editor</DialogDescription>
        </DialogHeader>

        <form
          className="grid min-h-0 gap-4 overflow-y-auto pe-1"
          id="routing-rule-form"
          onSubmit={(event) => {
            event.preventDefault();
            void onSubmit(formToRule(form));
          }}
        >
          <div className="grid gap-3 sm:grid-cols-[1fr_10rem_10rem]">
            <TextField
              label="Remarks"
              onChange={(value) => setForm((current) => ({ ...current, Remarks: value }))}
              value={form.Remarks}
            />
            <SelectField
              label="Rule type"
              onChange={(value) => setForm((current) => ({ ...current, RuleType: Number(value) }))}
              value={String(form.RuleType)}
            >
              <option value={RULE_TYPES.All}>All</option>
              <option value={RULE_TYPES.Routing}>Routing</option>
              <option value={RULE_TYPES.Dns}>DNS</option>
            </SelectField>
            <TextField
              label="Outbound"
              onChange={(value) => setForm((current) => ({ ...current, OutboundTag: value }))}
              value={form.OutboundTag}
            />
          </div>
          <div className="grid gap-3 sm:grid-cols-3">
            <TextField
              label="Port"
              onChange={(value) => setForm((current) => ({ ...current, Port: value }))}
              value={form.Port}
            />
            <TextField
              label="Network"
              onChange={(value) => setForm((current) => ({ ...current, Network: value }))}
              value={form.Network}
            />
            <TextField
              label="Type"
              onChange={(value) => setForm((current) => ({ ...current, Type: value }))}
              value={form.Type}
            />
          </div>
          <div className="grid gap-3 lg:grid-cols-2">
            <TextAreaField
              label="Domain"
              onChange={(value) => setForm((current) => ({ ...current, Domain: value }))}
              value={form.Domain}
            />
            <TextAreaField
              label="IP"
              onChange={(value) => setForm((current) => ({ ...current, Ip: value }))}
              value={form.Ip}
            />
            <TextAreaField
              label="Protocol"
              onChange={(value) => setForm((current) => ({ ...current, Protocol: value }))}
              value={form.Protocol}
            />
            <TextAreaField
              label="Process"
              onChange={(value) => setForm((current) => ({ ...current, Process: value }))}
              value={form.Process}
            />
            <TextAreaField
              label="Inbound tags"
              onChange={(value) => setForm((current) => ({ ...current, InboundTag: value }))}
              value={form.InboundTag}
            />
          </div>
          <label className="flex items-center gap-2 text-sm">
            <input
              checked={form.Enabled}
              onChange={(event) => setForm((current) => ({ ...current, Enabled: event.target.checked }))}
              type="checkbox"
            />
            Enabled
          </label>
        </form>

        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} type="button" variant="outline">
            Cancel
          </Button>
          <Button form="routing-rule-form" type="submit">
            <Save className="size-4" aria-hidden="true" />
            Save
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

type RuleFormState = {
  Id?: string;
  Domain: string;
  Enabled: boolean;
  InboundTag: string;
  Ip: string;
  Network: string;
  OutboundTag: string;
  Port: string;
  Process: string;
  Protocol: string;
  Remarks: string;
  RuleType: number;
  Type: string;
};

type RoutingFormState = {
  Id?: string;
  CustomIcon?: string;
  CustomRulesetPath4Singbox: string;
  DomainStrategy: string;
  DomainStrategy4Singbox: string;
  Enabled: boolean;
  IsActive?: boolean;
  Locked?: boolean;
  Remarks: string;
  RuleNum?: number;
  RuleSet: RulesItem_Serialize[];
  Sort?: number;
  Url: string;
};

type RulePayload = {
  Id?: string;
  Domain: string[] | null;
  Enabled: boolean;
  InboundTag: string[] | null;
  Ip: string[] | null;
  Network: string | null;
  OutboundTag: string | null;
  Port: string | null;
  Process: string[] | null;
  Protocol: string[] | null;
  Remarks: string | null;
  RuleType: number;
  Type: string | null;
};

function routingToForm(routing: RoutingItem_Serialize | null): RoutingFormState {
  return routing
    ? {
        CustomIcon: routing.CustomIcon,
        CustomRulesetPath4Singbox: routing.CustomRulesetPath4Singbox,
        DomainStrategy: routing.DomainStrategy || "AsIs",
        DomainStrategy4Singbox: routing.DomainStrategy4Singbox,
        Enabled: routing.Enabled,
        Id: routing.Id,
        IsActive: routing.IsActive,
        Locked: routing.Locked,
        Remarks: routing.Remarks,
        RuleNum: routing.RuleNum,
        RuleSet: routing.RuleSet,
        Sort: routing.Sort,
        Url: routing.Url,
      }
    : createDefaultRouting();
}

function createDefaultRouting(): RoutingFormState {
  return {
    CustomRulesetPath4Singbox: "",
    DomainStrategy: "AsIs",
    DomainStrategy4Singbox: "",
    Enabled: true,
    Remarks: "",
    RuleSet: [],
    Url: "",
  };
}

function ruleToForm(rule: RulesItem_Serialize | null): RuleFormState {
  return {
    Id: rule?.Id,
    Domain: listToText(rule?.Domain),
    Enabled: rule?.Enabled ?? true,
    InboundTag: listToText(rule?.InboundTag),
    Ip: listToText(rule?.Ip),
    Network: rule?.Network ?? "",
    OutboundTag: rule?.OutboundTag ?? "proxy",
    Port: rule?.Port ?? "",
    Process: listToText(rule?.Process),
    Protocol: listToText(rule?.Protocol),
    Remarks: rule?.Remarks ?? "",
    RuleType: rule?.RuleType ?? RULE_TYPES.Routing,
    Type: rule?.Type ?? "",
  };
}

function formToRule(form: RuleFormState): RulePayload {
  return {
    Id: form.Id,
    Domain: textToList(form.Domain),
    Enabled: form.Enabled,
    InboundTag: textToList(form.InboundTag),
    Ip: textToList(form.Ip),
    Network: emptyToNull(form.Network),
    OutboundTag: emptyToNull(form.OutboundTag),
    Port: emptyToNull(form.Port),
    Process: textToList(form.Process),
    Protocol: textToList(form.Protocol),
    Remarks: emptyToNull(form.Remarks),
    RuleType: form.RuleType,
    Type: emptyToNull(form.Type),
  };
}

function TextField({
  label,
  onChange,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  return (
    <label className="grid gap-1.5 text-sm">
      <span className="font-medium">{label}</span>
      <input
        className="h-9 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
    </label>
  );
}

function SelectField({
  children,
  label,
  onChange,
  value,
}: {
  children: React.ReactNode;
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  return (
    <label className="grid gap-1.5 text-sm">
      <span className="font-medium">{label}</span>
      <select
        className="h-9 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        {children}
      </select>
    </label>
  );
}

function TextAreaField({
  label,
  onChange,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  return (
    <label className="grid gap-1.5 text-sm">
      <span className="font-medium">{label}</span>
      <textarea
        className="min-h-24 resize-y rounded-md border bg-background px-3 py-2 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
    </label>
  );
}

function formatRuleType(ruleType: number | null | undefined) {
  switch (ruleType) {
    case RULE_TYPES.Routing:
      return "Routing";
    case RULE_TYPES.Dns:
      return "DNS";
    case RULE_TYPES.All:
    default:
      return "All";
  }
}

function formatList(values: string[] | null | undefined) {
  return values?.join(", ") ?? "";
}

function listToText(values: string[] | null | undefined) {
  return values?.join("\n") ?? "";
}

function textToList(value: string) {
  const list = value
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter(Boolean);

  return list.length > 0 ? list : null;
}

function emptyToNull(value: string) {
  const trimmed = value.trim();

  return trimmed ? trimmed : null;
}
