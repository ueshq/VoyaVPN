import { useEffect, useId, useMemo, useState } from "react";
import type * as React from "react";
import type {
  Control,
  UseFormGetValues,
  UseFormRegister,
  UseFormSetValue,
} from "react-hook-form";
import { useWatch } from "react-hook-form";
import { useQuery } from "@tanstack/react-query";
import {
  AlertTriangle,
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  Eye,
  Layers3,
  Plus,
  Search,
  X,
} from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
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
  listGroupChildCandidates,
  previewGroupProfile,
} from "@/ipc";
import type {
  GroupChildCandidate,
  GroupPreview,
  GroupPreviewRoute,
  ProfileItem_Deserialize,
} from "@/ipc/bindings";
import { cn } from "@/lib/utils";

import { CONFIG_TYPES, getProtocolLabel, type ProfileProtocol } from "@/features/profiles/profile-constants";
import {
  prepareGroupDraftForPreview,
  type ParsedProfileFormValues,
  type ProfileFormValues,
} from "@/features/profiles/profile-form-schema";

type GroupBuilderProps = {
  configType: ProfileProtocol;
  control: Control<ProfileFormValues, unknown, ParsedProfileFormValues>;
  getValues: UseFormGetValues<ProfileFormValues>;
  register: UseFormRegister<ProfileFormValues>;
  setValue: UseFormSetValue<ProfileFormValues>;
};

const multipleLoadOptions = [
  { label: "Least ping", value: 0 },
  { label: "Fallback", value: 1 },
  { label: "Random", value: 2 },
  { label: "Round robin", value: 3 },
  { label: "Least load", value: 4 },
];

export function GroupBuilder({
  configType,
  control,
  getValues,
  register,
  setValue,
}: GroupBuilderProps) {
  const [pickerOpen, setPickerOpen] = useState(false);
  const [preview, setPreview] = useState<GroupPreview | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const multipleLoadId = useId();
  const values = useWatch({ control }) as ProfileFormValues;
  const childItems = (values.ProtocolExtra?.ChildItems ?? "") as string;
  const currentIndexId = (values.IndexId ?? "") || null;
  const multipleLoad = values.ProtocolExtra?.MultipleLoad;
  const selectedIds = useMemo(() => splitIds(childItems), [childItems]);
  const candidatesQuery = useQuery({
    queryFn: () => listGroupChildCandidates(currentIndexId, null),
    queryKey: ["group-child-candidates", currentIndexId],
  });
  const candidates = useMemo(() => candidatesQuery.data ?? [], [candidatesQuery.data]);
  const candidatesById = useMemo(
    () => new Map(candidates.map((candidate) => [candidate.indexId, candidate])),
    [candidates],
  );
  const isProxyChain = configType === CONFIG_TYPES.ProxyChain;
  const [pickerDraftIds, setPickerDraftIds] = useState<string[]>(selectedIds);

  useEffect(() => {
    const groupType = isProxyChain ? "ProxyChain" : "PolicyGroup";
    if (values.ProtocolExtra?.GroupType !== groupType) {
      setValue("ProtocolExtra.GroupType", groupType, { shouldDirty: true });
    }
  }, [isProxyChain, setValue, values.ProtocolExtra?.GroupType]);

  useEffect(() => {
    if (multipleLoad == null) {
      setValue("ProtocolExtra.MultipleLoad", 0);
    }
  }, [multipleLoad, setValue]);

  function setSelectedIds(ids: string[]) {
    setValue("ProtocolExtra.ChildItems", ids.join(","), {
      shouldDirty: true,
      shouldValidate: true,
    });
    setPreview(null);
  }

  function removeChild(indexId: string) {
    setSelectedIds(selectedIds.filter((id) => id !== indexId));
  }

  function moveChild(indexId: string, direction: -1 | 1) {
    const index = selectedIds.indexOf(indexId);
    const nextIndex = index + direction;
    if (index < 0 || nextIndex < 0 || nextIndex >= selectedIds.length) {
      return;
    }

    const next = [...selectedIds];
    [next[index], next[nextIndex]] = [next[nextIndex], next[index]];
    setSelectedIds(next);
  }

  async function loadPreview() {
    setPreviewLoading(true);
    setPreviewError(null);
    try {
      const draft = prepareGroupDraftForPreview(getValues()) as ProfileItem_Deserialize;
      const nextPreview = await previewGroupProfile(draft);
      setPreview(nextPreview);
    } catch (error) {
      setPreviewError(error instanceof Error ? error.message : String(error));
    } finally {
      setPreviewLoading(false);
    }
  }

  return (
    <div className="grid gap-4">
      <div className="grid gap-3 lg:grid-cols-[1fr_1fr_10rem]">
        <LabeledField label={isProxyChain ? "Chain marker" : "Group marker"} {...register("ProtocolExtra.GroupType")} />
        <LabeledField label="Subscription child group" {...register("ProtocolExtra.SubChildItems")} />
        <div className="grid gap-1.5">
          <Label htmlFor={multipleLoadId}>Load mode</Label>
          <Select
            onValueChange={(value) => {
              setValue("ProtocolExtra.MultipleLoad", optionalNumber(value) ?? 0, {
                shouldDirty: true,
                shouldValidate: true,
              });
            }}
            value={String(multipleLoad ?? 0)}
          >
            <SelectTrigger className="w-full" id={multipleLoadId}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {multipleLoadOptions.map((option) => (
                <SelectItem key={option.value} value={String(option.value)}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      <div className="grid gap-3 lg:grid-cols-[1fr_13rem]">
        <LabeledField label="Subscription filter" placeholder="^US|Japan" {...register("ProtocolExtra.Filter")} />
        <div className="flex items-end">
          <Button
            className="w-full"
            onClick={() => {
              setPickerDraftIds(selectedIds);
              setPickerOpen(true);
            }}
            type="button"
            variant="outline"
          >
            <Plus className="size-4" aria-hidden="true" />
            Choose children
          </Button>
        </div>
      </div>

      <div className="grid gap-2">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2 text-sm font-medium">
            <Layers3 className="size-4 text-muted-foreground" aria-hidden="true" />
            Selected children
          </div>
          <Button disabled={previewLoading} onClick={() => void loadPreview()} size="sm" type="button" variant="outline">
            <Eye className="size-4" aria-hidden="true" />
            Preview
          </Button>
        </div>

        {selectedIds.length === 0 ? (
          <div className="rounded-lg border border-dashed bg-card px-3 py-6 text-center text-sm text-muted-foreground">
            No child profiles selected
          </div>
        ) : (
          <div className="grid gap-2">
            {selectedIds.map((indexId, index) => {
              const candidate = candidatesById.get(indexId);

              return (
                <div className="grid grid-cols-[1.5rem_1fr_auto] items-center gap-2 rounded-lg border bg-card px-3 py-2" key={`${indexId}-${index}`}>
                  <span className="text-xs tabular-nums text-muted-foreground">{index + 1}</span>
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium">
                      {candidate?.remarks || indexId}
                    </div>
                    <div className="truncate text-xs text-muted-foreground">
                      {candidate ? `${getProtocolLabel(candidate.configType)} · ${candidate.address || "group"}` : indexId}
                    </div>
                  </div>
                  <div className="flex items-center gap-1">
                    <IconButton
                      disabled={index === 0}
                      label="Move child up"
                      onClick={() => moveChild(indexId, -1)}
                    >
                      <ArrowUp className="size-4" aria-hidden="true" />
                    </IconButton>
                    <IconButton
                      disabled={index + 1 === selectedIds.length}
                      label="Move child down"
                      onClick={() => moveChild(indexId, 1)}
                    >
                      <ArrowDown className="size-4" aria-hidden="true" />
                    </IconButton>
                    <IconButton label="Remove child" onClick={() => removeChild(indexId)}>
                      <X className="size-4" aria-hidden="true" />
                    </IconButton>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {previewError ? (
        <ValidationMessage tone="error" messages={[previewError]} />
      ) : null}
      {preview ? <GroupPreviewPanel preview={preview} /> : null}

      <ServerPickerDialog
        candidates={candidates}
        draftIds={pickerDraftIds}
        loading={candidatesQuery.isLoading}
        onOpenChange={setPickerOpen}
        onDraftIdsChange={setPickerDraftIds}
        onSelected={setSelectedIds}
        open={pickerOpen}
      />
    </div>
  );
}

function ServerPickerDialog({
  candidates,
  draftIds,
  loading,
  onDraftIdsChange,
  onOpenChange,
  onSelected,
  open,
}: {
  candidates: GroupChildCandidate[];
  draftIds: string[];
  loading: boolean;
  onDraftIdsChange: React.Dispatch<React.SetStateAction<string[]>>;
  onOpenChange: (open: boolean) => void;
  onSelected: (ids: string[]) => void;
  open: boolean;
}) {
  const [filter, setFilter] = useState("");
  const searchId = useId();
  const pickerId = useId();
  const filtered = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    if (!needle) {
      return candidates;
    }

    return candidates.filter((candidate) =>
      [candidate.remarks, candidate.address, candidate.indexId, getProtocolLabel(candidate.configType)]
        .join(" ")
        .toLowerCase()
        .includes(needle),
    );
  }, [candidates, filter]);

  function toggleCandidate(candidate: GroupChildCandidate, selected: boolean) {
    if (!candidate.selectable) {
      return;
    }
    onDraftIdsChange((current) => {
      if (selected) {
        return current.includes(candidate.indexId) ? current : [...current, candidate.indexId];
      }

      return current.filter((indexId) => indexId !== candidate.indexId);
    });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[86vh] w-[min(94vw,54rem)] grid-rows-[auto_auto_minmax(0,1fr)_auto] overflow-hidden">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Layers3 className="size-4" aria-hidden="true" />
            Select child profiles
          </DialogTitle>
          <DialogDescription className="sr-only">
            Choose servers, policy groups, or proxy chains for the current group.
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-1.5">
          <Label className="sr-only" htmlFor={searchId}>
            Filter child profiles
          </Label>
          <div className="relative">
            <Search className="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
            <Input
              className="ps-9"
              id={searchId}
              name="group-child-filter"
              type="search"
              autoComplete="off"
              aria-controls={pickerId}
              aria-label="Filter child profiles"
              onChange={(event) => setFilter(event.target.value)}
              placeholder="Filter child profiles"
              value={filter}
            />
          </div>
        </div>

        <ScrollArea className="min-h-0 overflow-hidden rounded-md border" id={pickerId}>
          {loading ? (
            <div className="grid h-40 place-items-center text-sm text-muted-foreground">Loading profiles</div>
          ) : filtered.length === 0 ? (
            <div className="grid h-40 place-items-center text-sm text-muted-foreground">No matching profiles</div>
          ) : (
            <div className="divide-y">
              {filtered.map((candidate) => {
                const checked = draftIds.includes(candidate.indexId);
                const checkboxId = `${pickerId}-${toDomId(candidate.indexId)}`;

                return (
                  <Label
                    className={cn(
                      "grid cursor-default grid-cols-[1.25rem_minmax(0,1fr)_auto] items-center gap-3 px-3 py-2 text-sm leading-normal",
                      candidate.selectable ? "hover:bg-accent" : "text-muted-foreground",
                    )}
                    htmlFor={checkboxId}
                    key={candidate.indexId}
                  >
                    <Checkbox
                      checked={checked}
                      disabled={!candidate.selectable}
                      id={checkboxId}
                      onCheckedChange={(nextChecked) => toggleCandidate(candidate, nextChecked === true)}
                    />
                    <span className="min-w-0">
                      <span className="block truncate font-medium">{candidate.remarks || candidate.indexId}</span>
                      <span className="block truncate text-xs text-muted-foreground">
                        {candidate.indexId} · {candidate.address || "group"}
                      </span>
                    </span>
                    <Badge className="justify-self-end bg-background text-muted-foreground" variant="outline">
                      {candidate.isGroup ? "Nested" : getProtocolLabel(candidate.configType)}
                    </Badge>
                  </Label>
                );
              })}
            </div>
          )}
        </ScrollArea>

        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} type="button" variant="outline">
            Cancel
          </Button>
          <Button
            onClick={() => {
              onSelected(draftIds);
              onOpenChange(false);
            }}
            type="button"
          >
            Apply
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function GroupPreviewPanel({ preview }: { preview: GroupPreview }) {
  const validation = preview.validation ?? {
    childIndexIds: [],
    errors: [],
    normalizedChildItems: "",
    valid: false,
    warnings: [],
  };
  const xrayRoutes = preview.xrayRoutes ?? [];
  const xrayBalancers = preview.xrayBalancers ?? [];
  const xrayObservatorySelectors = preview.xrayObservatorySelectors ?? [];
  const xrayBurstObservatorySelectors = preview.xrayBurstObservatorySelectors ?? [];
  const singboxRoutes = preview.singboxRoutes ?? [];

  return (
    <Card className="grid gap-3 rounded-lg bg-muted/30 p-3 shadow-none">
      {validation.valid ? (
        <div className="flex items-center gap-2 text-sm text-emerald-700 dark:text-emerald-300">
          <CheckCircle2 className="size-4" aria-hidden="true" />
          Preview generated from validated children
        </div>
      ) : (
        <ValidationMessage tone="error" messages={validation.errors ?? []} />
      )}
      {(validation.warnings ?? []).length > 0 ? (
        <ValidationMessage tone="warning" messages={validation.warnings ?? []} />
      ) : null}

      <div className="grid gap-3 xl:grid-cols-2">
        <PreviewList
          routes={xrayRoutes}
          title="Xray dialerProxy"
          details={[
            xrayBalancers.length > 0
              ? `balancer ${xrayBalancers.map((balancer) => balancer.tag).join(", ")}`
              : "",
            xrayObservatorySelectors.length > 0
              ? `observatory ${xrayObservatorySelectors.join(", ")}`
              : "",
            xrayBurstObservatorySelectors.length > 0
              ? `burst ${xrayBurstObservatorySelectors.join(", ")}`
              : "",
          ].filter(Boolean)}
        />
        <PreviewList routes={singboxRoutes} title="sing-box selector/urltest + detour" />
      </div>
    </Card>
  );
}

function PreviewList({
  details = [],
  routes,
  title,
}: {
  details?: string[];
  routes: GroupPreviewRoute[];
  title: string;
}) {
  return (
    <section className="grid gap-2">
      <h4 className="text-sm font-medium">{title}</h4>
      {details.length > 0 ? (
        <div className="flex flex-wrap gap-1">
          {details.map((detail) => (
            <Badge className="bg-background text-muted-foreground" key={detail} variant="outline">
              {detail}
            </Badge>
          ))}
        </div>
      ) : null}
      <ScrollArea className="h-48 rounded-md border bg-background">
        {routes.length === 0 ? (
          <div className="px-3 py-6 text-center text-sm text-muted-foreground">No generated routes</div>
        ) : (
          <div className="divide-y">
            {routes.map((route) => (
              <div className="grid gap-1 px-3 py-2 text-xs" key={`${route.tag}-${route.kind}`}>
                <div className="flex min-w-0 items-center gap-2">
                  <span className="truncate font-medium">{route.tag}</span>
                  <Badge className="rounded-sm px-1.5 py-0 text-muted-foreground" variant="secondary">
                    {route.kind}
                  </Badge>
                </div>
                <div className="truncate text-muted-foreground">
                  {route.dialerProxy ? `dialerProxy -> ${route.dialerProxy}` : null}
                  {route.detour ? `detour -> ${route.detour}` : null}
                  {route.outbounds.length > 0 ? `outbounds -> ${route.outbounds.join(", ")}` : null}
                  {route.downloadDialerProxy ? `download dialerProxy -> ${route.downloadDialerProxy}` : null}
                </div>
              </div>
            ))}
          </div>
        )}
      </ScrollArea>
    </section>
  );
}

function ValidationMessage({
  messages,
  tone,
}: {
  messages: string[];
  tone: "error" | "warning";
}) {
  if (messages.length === 0) {
    return null;
  }

  return (
    <Alert
      className={cn(
        "gap-1 px-3 py-2",
        tone === "error" ? "border-destructive/40 bg-destructive/10 text-destructive" : "border-amber-500/40 bg-amber-500/10 text-amber-700 dark:text-amber-200",
      )}
      role={tone === "error" ? "alert" : "status"}
      variant={tone === "error" ? "destructive" : "default"}
    >
      <AlertTriangle className="size-4" aria-hidden="true" />
      <AlertTitle>
        {tone === "error" ? "Validation failed" : "Validation warnings"}
      </AlertTitle>
      <AlertDescription>
        {messages.map((message) => (
          <div key={message}>{message}</div>
        ))}
      </AlertDescription>
    </Alert>
  );
}

function LabeledField({
  className,
  id,
  label,
  ...props
}: React.InputHTMLAttributes<HTMLInputElement> & { label: string }) {
  const generatedId = useId();
  const inputId = id ?? generatedId;

  return (
    <div className="grid gap-1.5">
      <Label htmlFor={inputId}>{label}</Label>
      <Input className={className} id={inputId} {...props} />
    </div>
  );
}

function IconButton({
  children,
  disabled,
  label,
  onClick,
}: {
  children: React.ReactNode;
  disabled?: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button
      aria-label={label}
      className="size-8 p-0"
      disabled={disabled}
      onClick={onClick}
      title={label}
      type="button"
      variant="ghost"
    >
      {children}
    </Button>
  );
}

function splitIds(value?: string | null) {
  return (value ?? "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function optionalNumber(value: unknown) {
  if (value === "" || value == null) {
    return undefined;
  }

  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

function toDomId(value: string) {
  return value.replace(/[^A-Za-z0-9_-]/g, "_");
}
