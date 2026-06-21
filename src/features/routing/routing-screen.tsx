import { useId, useMemo, useState } from "react";
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
  TriangleAlert,
  Trash2,
} from "lucide-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { PageHeader, PageHeaderHeading, PageSection } from "@/components/app-shell/page-section";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Textarea } from "@/components/ui/textarea";
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
  RoutingItem_Serialize,
  RulesItem_Serialize,
} from "@/ipc/bindings";
import { cn } from "@/lib/utils";
import { z } from "zod";

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

const DOMAIN_STRATEGIES = ["AsIs", "IPIfNonMatch", "IPOnDemand"] as const;
const SINGBOX_DOMAIN_STRATEGIES = ["", "prefer_ipv4", "prefer_ipv6", "ipv4_only", "ipv6_only"] as const;

const optionalNullableText = z.string().trim().nullable().optional();
const optionalStringList = z.array(z.string().trim().min(1, "List items cannot be empty")).nullable().optional();
const routingRuleSchema = z.object({
  Id: z.string().trim().optional(),
  Type: optionalNullableText,
  Port: optionalNullableText.superRefine(validatePortExpression),
  Network: optionalNullableText.superRefine(validateNetworkExpression),
  InboundTag: optionalStringList,
  OutboundTag: optionalNullableText,
  Ip: optionalStringList,
  Domain: optionalStringList,
  Protocol: optionalStringList,
  Process: optionalStringList,
  Enabled: z.boolean().optional(),
  Remarks: optionalNullableText,
  RuleType: z.union([z.literal(RULE_TYPES.All), z.literal(RULE_TYPES.Routing), z.literal(RULE_TYPES.Dns)]).nullable().optional(),
});
const optionalHttpsUrl = z.string().trim().superRefine(validateHttpsUrl);
const routingProfileSchema = z.object({
  Id: z.string().trim().optional(),
  CustomIcon: z.string().optional(),
  CustomRulesetPath4Singbox: z.string().trim(),
  DomainStrategy: z.enum(DOMAIN_STRATEGIES),
  DomainStrategy4Singbox: z.enum(SINGBOX_DOMAIN_STRATEGIES),
  Enabled: z.boolean(),
  IsActive: z.boolean().optional(),
  Locked: z.boolean().optional(),
  Remarks: z.string().trim().max(256, "Remarks must be 256 characters or fewer"),
  RuleNum: z.number().int().nonnegative().optional(),
  RuleSet: z.array(routingRuleSchema),
  Sort: z.number().int().optional(),
  Url: optionalHttpsUrl,
});
const routingTemplateUrlSchema = optionalHttpsUrl;

type ErrorMap = Record<string, string>;
type RoutingFormPayload = z.output<typeof routingProfileSchema>;
type RoutingRulePayload = z.output<typeof routingRuleSchema>;

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
  const [templateUrlError, setTemplateUrlError] = useState<string | null>(null);
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
      return true;
    } catch (error) {
      setOperationError(error instanceof Error ? error.message : String(error));
      return false;
    }
  }

  async function saveTemplateUrl(config: AppConfig_Serialize | undefined, url: string) {
    const current = config ?? (await loadAppConfig());
    await saveAppConfig({
      ...current,
      ConstItem: {
        ...current.ConstItem,
        RouteRulesTemplateSourceUrl: url || null,
      },
    });
    setTemplateUrlDraft(url);
    await queryClient.invalidateQueries({ queryKey: ["app-config"] });
  }

  async function handleImportTemplates() {
    const parsedTemplateUrl = routingTemplateUrlSchema.safeParse(templateUrl);
    if (!parsedTemplateUrl.success) {
      setTemplateUrlError(firstZodMessage(parsedTemplateUrl.error));
      setOperationError("Routing template URL validation failed");
      return;
    }

    setTemplateUrlError(null);
    await runOperation(async () => {
      await saveTemplateUrl(configQuery.data, parsedTemplateUrl.data);
      await importRoutingTemplates(false, null, false);
    });
  }

  async function handleSaveRouting(routing: RoutingFormPayload) {
    const saved = await runOperation(async () => {
      const saved = await saveRouting(routing);
      setSelectedRoutingId(saved.Id);
    });
    if (saved) {
      setRoutingDialog(null);
    }
  }

  async function handleSaveRule(rule: RoutingRulePayload) {
    if (!selectedRouting) {
      return;
    }
    const saved = await runOperation(async () => {
      const saved = await saveRoutingRule(selectedRouting.Id, rule);
      setSelectedRoutingId(saved.Id);
      setSelectedRuleId(rule.Id ?? saved.RuleSet.at(-1)?.Id ?? null);
    });
    if (saved) {
      setRuleDialog(null);
    }
  }

  return (
    <PageSection aria-label="Routing">
      <PageHeader>
        <PageHeaderHeading icon={Route} title="Routing">
          <Badge variant="outline">{routings.length.toLocaleString()} profiles</Badge>
        </PageHeaderHeading>

        <div className="ms-auto min-w-[18rem] max-w-xl flex-1 md:flex-none">
          <Label className="sr-only" htmlFor="routing-template-url">
            Template URL
          </Label>
          <div className="relative">
            <Globe2
              className="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
              aria-hidden="true"
            />
            <Input
              aria-describedby={templateUrlError ? "routing-template-url-error" : undefined}
              aria-invalid={templateUrlError ? true : undefined}
              className="ps-9"
              id="routing-template-url"
              onChange={(event) => {
                setTemplateUrlDraft(event.target.value);
                setTemplateUrlError(null);
              }}
              placeholder="RouteRulesTemplateSourceUrl"
              value={templateUrl}
            />
          </div>
          {templateUrlError ? (
            <span className="text-xs text-destructive" id="routing-template-url-error">
              {templateUrlError}
            </span>
          ) : null}
        </div>
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
      </PageHeader>

      {operationError ? (
        <div className="border-b px-4 py-2">
          <Alert className="py-2" variant="destructive">
            <TriangleAlert aria-hidden="true" />
            <AlertDescription>{operationError}</AlertDescription>
          </Alert>
        </div>
      ) : null}

      <div className="grid min-h-0 flex-1 grid-cols-1 lg:grid-cols-[21rem_1fr]">
        <aside className="min-h-0 border-b lg:border-b-0 lg:border-e">
          <div className="h-10 border-b px-4 py-2 text-xs font-medium uppercase text-muted-foreground">
            Profiles
          </div>
          <ScrollArea className="h-[18rem] lg:h-full">
            {routings.length > 0 ? (
              routings.map((routing) => (
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
                  {routing.IsActive ? (
                    <Badge className="shrink-0" variant="secondary">
                      Active
                    </Badge>
                  ) : null}
                </button>
              ))
            ) : (
              <div className="grid h-full place-items-center px-4 py-8 text-center text-sm text-muted-foreground">
                No routing profiles
              </div>
            )}
          </ScrollArea>
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

          <ScrollArea className="min-h-0 flex-1">
            <Table className="min-w-[58rem]">
              <TableHeader className="sticky top-0 z-10 bg-background text-xs">
                <TableRow className="hover:bg-transparent">
                  <TableHead className="w-12 px-3 text-muted-foreground" scope="col">
                    #
                  </TableHead>
                  <TableHead className="px-3 text-muted-foreground" scope="col">
                    Remarks
                  </TableHead>
                  <TableHead className="px-3 text-muted-foreground" scope="col">
                    Outbound
                  </TableHead>
                  <TableHead className="px-3 text-muted-foreground" scope="col">
                    Type
                  </TableHead>
                  <TableHead className="px-3 text-muted-foreground" scope="col">
                    Domain
                  </TableHead>
                  <TableHead className="px-3 text-muted-foreground" scope="col">
                    IP
                  </TableHead>
                  <TableHead className="px-3 text-muted-foreground" scope="col">
                    Port
                  </TableHead>
                  <TableHead className="px-3 text-muted-foreground" scope="col">
                    Network
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {(selectedRouting?.RuleSet ?? []).length > 0 ? (
                  (selectedRouting?.RuleSet ?? []).map((rule, index) => (
                    <TableRow
                      className={cn(
                        "cursor-default hover:bg-accent/70",
                        selectedRule?.Id === rule.Id ? "bg-accent" : "",
                        !rule.Enabled ? "opacity-55" : "",
                      )}
                      key={rule.Id}
                      onClick={() => setSelectedRuleId(rule.Id)}
                    >
                      <TableCell className="px-3 py-2 tabular-nums text-muted-foreground">{index + 1}</TableCell>
                      <TableCell className="max-w-52 truncate px-3 py-2 font-medium">{rule.Remarks ?? ""}</TableCell>
                      <TableCell className="px-3 py-2">{rule.OutboundTag ?? ""}</TableCell>
                      <TableCell className="px-3 py-2">
                        <RuleTypeBadge ruleType={rule.RuleType} />
                      </TableCell>
                      <TableCell className="max-w-72 truncate px-3 py-2">{formatList(rule.Domain)}</TableCell>
                      <TableCell className="max-w-56 truncate px-3 py-2">{formatList(rule.Ip)}</TableCell>
                      <TableCell className="px-3 py-2">{rule.Port ?? ""}</TableCell>
                      <TableCell className="px-3 py-2">{rule.Network ?? ""}</TableCell>
                    </TableRow>
                  ))
                ) : (
                  <TableRow className="hover:bg-transparent">
                    <TableCell className="h-28 text-center text-muted-foreground" colSpan={8}>
                      No routing rules
                    </TableCell>
                  </TableRow>
                )}
              </TableBody>
            </Table>
          </ScrollArea>
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
    </PageSection>
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
  onSubmit: (routing: RoutingFormPayload) => Promise<void>;
  open: boolean;
  routing: RoutingItem_Serialize | null;
}) {
  const [form, setForm] = useState<RoutingFormState>(() => routingToForm(routing));
  const [fieldErrors, setFieldErrors] = useState<ErrorMap>({});

  async function submitForm(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      const payload = routingProfileSchema.parse(form);
      setFieldErrors({});
      await onSubmit(payload);
    } catch (error) {
      if (error instanceof z.ZodError) {
        setFieldErrors(zodIssuesToErrorMap(error));
        return;
      }
      throw error;
    }
  }

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
          onSubmit={(event) => void submitForm(event)}
        >
          <TextField
            error={fieldErrors.Remarks}
            label="Remarks"
            onChange={(value) => setForm((current) => ({ ...current, Remarks: value }))}
            value={form.Remarks ?? ""}
          />
          <div className="grid gap-3 sm:grid-cols-2">
            <SelectField
              error={fieldErrors.DomainStrategy}
              label="Xray domain strategy"
              onChange={(value) => setForm((current) => ({ ...current, DomainStrategy: value }))}
              options={DOMAIN_STRATEGIES.map((strategy) => ({ label: strategy, value: strategy }))}
              value={form.DomainStrategy ?? "AsIs"}
            />
            <SelectField
              error={fieldErrors.DomainStrategy4Singbox}
              label="sing-box domain strategy"
              onChange={(value) => setForm((current) => ({ ...current, DomainStrategy4Singbox: value }))}
              options={SINGBOX_DOMAIN_STRATEGIES.map((strategy) => ({
                label: strategy || "default",
                value: strategy,
              }))}
              value={form.DomainStrategy4Singbox ?? ""}
            />
          </div>
          <TextField
            error={fieldErrors.CustomRulesetPath4Singbox}
            label="Ruleset path for sing-box"
            onChange={(value) => setForm((current) => ({ ...current, CustomRulesetPath4Singbox: value }))}
            value={form.CustomRulesetPath4Singbox ?? ""}
          />
          <TextField
            error={fieldErrors.Url}
            label="Source URL"
            onChange={(value) => setForm((current) => ({ ...current, Url: value }))}
            value={form.Url ?? ""}
          />
          <CheckboxField
            checked={form.Enabled ?? true}
            label="Enabled"
            onCheckedChange={(checked) => setForm((current) => ({ ...current, Enabled: checked }))}
          />
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
  onSubmit: (rule: RoutingRulePayload) => Promise<void>;
  open: boolean;
  rule: RulesItem_Serialize | null;
}) {
  const [form, setForm] = useState<RuleFormState>(() => ruleToForm(rule));
  const [fieldErrors, setFieldErrors] = useState<ErrorMap>({});

  async function submitForm(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      const payload = routingRuleSchema.parse(formToRule(form));
      setFieldErrors({});
      await onSubmit(payload);
    } catch (error) {
      if (error instanceof z.ZodError) {
        setFieldErrors(zodIssuesToErrorMap(error));
        return;
      }
      throw error;
    }
  }

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
          onSubmit={(event) => void submitForm(event)}
        >
          <div className="grid gap-3 sm:grid-cols-[1fr_10rem_10rem]">
            <TextField
              error={fieldErrors.Remarks}
              label="Remarks"
              onChange={(value) => setForm((current) => ({ ...current, Remarks: value }))}
              value={form.Remarks}
            />
            <SelectField
              error={fieldErrors.RuleType}
              label="Rule type"
              onChange={(value) => setForm((current) => ({ ...current, RuleType: Number(value) }))}
              options={[
                { label: "All", value: String(RULE_TYPES.All) },
                { label: "Routing", value: String(RULE_TYPES.Routing) },
                { label: "DNS", value: String(RULE_TYPES.Dns) },
              ]}
              value={String(form.RuleType)}
            />
            <TextField
              error={fieldErrors.OutboundTag}
              label="Outbound"
              onChange={(value) => setForm((current) => ({ ...current, OutboundTag: value }))}
              value={form.OutboundTag}
            />
          </div>
          <div className="grid gap-3 sm:grid-cols-3">
            <TextField
              error={fieldErrors.Port}
              label="Port"
              onChange={(value) => setForm((current) => ({ ...current, Port: value }))}
              value={form.Port}
            />
            <TextField
              error={fieldErrors.Network}
              label="Network"
              onChange={(value) => setForm((current) => ({ ...current, Network: value }))}
              value={form.Network}
            />
            <TextField
              error={fieldErrors.Type}
              label="Type"
              onChange={(value) => setForm((current) => ({ ...current, Type: value }))}
              value={form.Type}
            />
          </div>
          <div className="grid gap-3 lg:grid-cols-2">
            <TextAreaField
              error={fieldErrors.Domain}
              label="Domain"
              onChange={(value) => setForm((current) => ({ ...current, Domain: value }))}
              value={form.Domain}
            />
            <TextAreaField
              error={fieldErrors.Ip}
              label="IP"
              onChange={(value) => setForm((current) => ({ ...current, Ip: value }))}
              value={form.Ip}
            />
            <TextAreaField
              error={fieldErrors.Protocol}
              label="Protocol"
              onChange={(value) => setForm((current) => ({ ...current, Protocol: value }))}
              value={form.Protocol}
            />
            <TextAreaField
              error={fieldErrors.Process}
              label="Process"
              onChange={(value) => setForm((current) => ({ ...current, Process: value }))}
              value={form.Process}
            />
            <TextAreaField
              error={fieldErrors.InboundTag}
              label="Inbound tags"
              onChange={(value) => setForm((current) => ({ ...current, InboundTag: value }))}
              value={form.InboundTag}
            />
          </div>
          <CheckboxField
            checked={form.Enabled}
            label="Enabled"
            onCheckedChange={(checked) => setForm((current) => ({ ...current, Enabled: checked }))}
          />
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

type SelectOption = {
  label: string;
  value: string;
};

const EMPTY_SELECT_VALUE = "__voyavpn_empty_select_value__";

function TextField({
  error,
  label,
  onChange,
  value,
}: {
  error?: string;
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  const id = useId();
  const errorId = `${id}-error`;

  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Input
        aria-describedby={error ? errorId : undefined}
        aria-invalid={error ? true : undefined}
        id={id}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
      {error ? (
        <span className="text-xs text-destructive" id={errorId}>
          {error}
        </span>
      ) : null}
    </div>
  );
}

function SelectField({
  error,
  label,
  onChange,
  options,
  value,
}: {
  error?: string;
  label: string;
  onChange: (value: string) => void;
  options: SelectOption[];
  value: string;
}) {
  const id = useId();
  const errorId = `${id}-error`;
  const selectValue = value === "" ? EMPTY_SELECT_VALUE : value;

  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Select
        onValueChange={(nextValue) => onChange(nextValue === EMPTY_SELECT_VALUE ? "" : nextValue)}
        value={selectValue}
      >
        <SelectTrigger
          aria-describedby={error ? errorId : undefined}
          aria-invalid={error ? true : undefined}
          className="w-full"
          id={id}
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem
              key={option.value || EMPTY_SELECT_VALUE}
              value={option.value === "" ? EMPTY_SELECT_VALUE : option.value}
            >
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {error ? (
        <span className="text-xs text-destructive" id={errorId}>
          {error}
        </span>
      ) : null}
    </div>
  );
}

function TextAreaField({
  error,
  label,
  onChange,
  value,
}: {
  error?: string;
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  const id = useId();
  const errorId = `${id}-error`;

  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Textarea
        aria-describedby={error ? errorId : undefined}
        aria-invalid={error ? true : undefined}
        className="min-h-24 resize-y"
        id={id}
        onChange={(event) => onChange(event.target.value)}
        value={value}
      />
      {error ? (
        <span className="text-xs text-destructive" id={errorId}>
          {error}
        </span>
      ) : null}
    </div>
  );
}

function CheckboxField({
  checked,
  label,
  onCheckedChange,
}: {
  checked: boolean;
  label: string;
  onCheckedChange: (checked: boolean) => void;
}) {
  const id = useId();

  return (
    <div className="flex items-center gap-2">
      <Checkbox
        checked={checked}
        id={id}
        onCheckedChange={(nextChecked) => onCheckedChange(nextChecked === true)}
      />
      <Label htmlFor={id}>{label}</Label>
    </div>
  );
}

function RuleTypeBadge({ ruleType }: { ruleType: number | null | undefined }) {
  return (
    <Badge className="bg-background" variant="outline">
      {formatRuleType(ruleType)}
    </Badge>
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

function validateHttpsUrl(value: string, context: z.RefinementCtx) {
  if (value === "") {
    return;
  }

  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch (error) {
    context.addIssue({
      code: "custom",
      message: `URL must be valid: ${error instanceof Error ? error.message : String(error)}`,
    });
    return;
  }

  if (parsed.protocol !== "https:") {
    context.addIssue({ code: "custom", message: "URL must use https://" });
  }
  if (!parsed.hostname) {
    context.addIssue({ code: "custom", message: "URL host is required" });
  }
  if (parsed.username || parsed.password) {
    context.addIssue({ code: "custom", message: "URL must not include credentials" });
  }
}

function validatePortExpression(value: string | null | undefined, context: z.RefinementCtx) {
  if (!value) {
    return;
  }

  for (const token of value.split(",")) {
    const part = token.trim();
    const match = /^(\d{1,5})(?:-(\d{1,5}))?$/.exec(part);
    if (!match) {
      context.addIssue({
        code: "custom",
        message: "Port must be a comma-separated list of ports or ranges",
      });
      return;
    }

    const start = Number(match[1]);
    const end = match[2] ? Number(match[2]) : start;
    if (start > 65535 || end > 65535 || start > end) {
      context.addIssue({
        code: "custom",
        message: "Port values must be between 0 and 65535 and ranges must ascend",
      });
      return;
    }
  }
}

function validateNetworkExpression(value: string | null | undefined, context: z.RefinementCtx) {
  if (!value) {
    return;
  }

  const allowed = new Set(["tcp", "udp"]);
  const values = value
    .split(",")
    .map((item) => item.trim().toLowerCase())
    .filter(Boolean);
  if (values.length === 0 || values.some((item) => !allowed.has(item))) {
    context.addIssue({ code: "custom", message: "Network must be tcp, udp, or tcp,udp" });
  }
}

function zodIssuesToErrorMap(error: z.ZodError): ErrorMap {
  return Object.fromEntries(
    error.issues.map((issue) => [issue.path.join(".") || "form", issue.message]),
  );
}

function firstZodMessage(error: z.ZodError) {
  return error.issues[0]?.message ?? "Validation failed";
}
