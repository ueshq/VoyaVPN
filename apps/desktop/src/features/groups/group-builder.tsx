import { useEffect, useId, useMemo, useRef, useState } from "react";
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

import { Alert, AlertDescription, AlertTitle } from "@voya/ui/components/alert";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { Card } from "@voya/ui/components/card";
import { Checkbox } from "@voya/ui/components/checkbox";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { Input } from "@voya/ui/components/input";
import { Label } from "@voya/ui/components/label";
import { ScrollArea } from "@voya/ui/components/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@voya/ui/components/select";
import {
  listGroupChildCandidates,
  previewGroupProfile,
} from "@/ipc";
import type {
  GroupChildCandidate,
  GroupPreview,
  GroupPreviewRoute,
  LoadStrategy,
} from "@/ipc/bindings";
import { groupChildCandidatesQueryKey } from "@/ipc/query-keys";
import { useMountedRef } from "@voya/utils/use-mounted-ref";
import { getErrorMessage } from "@voya/utils/error";
import { cn } from "@voya/ui/lib/utils";
import { useI18n } from "@voya/i18n/use-i18n";

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

type Translation = ReturnType<typeof useI18n>["t"];

function loadStrategyOptions(t: Translation) {
  return [
    { label: t("panes.groups.loadLeastPing"), value: "leastPing" },
    { label: t("panes.groups.loadFallback"), value: "fallback" },
    { label: t("panes.groups.loadRandom"), value: "random" },
    { label: t("panes.groups.loadRoundRobin"), value: "roundRobin" },
    { label: t("panes.groups.loadLeastLoad"), value: "leastLoad" },
  ] as const satisfies ReadonlyArray<{ label: string; value: LoadStrategy }>;
}

export function GroupBuilder({
  configType,
  control,
  getValues,
  register,
  setValue,
}: GroupBuilderProps) {
  const { t } = useI18n();
  const [pickerOpen, setPickerOpen] = useState(false);
  const [preview, setPreview] = useState<GroupPreview | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const previewRequestId = useRef(0);
  const mountedRef = useMountedRef();
  const multipleLoadId = useId();
  const values = useWatch({ control }) as ProfileFormValues;
  const childItems = (values.protocolOptions?.childProfileIds ?? "") as string;
  const currentIndexId = (values.indexId ?? "") || null;
  const multipleLoad = values.protocolOptions?.loadStrategy;
  const selectedIds = useMemo(() => splitIds(childItems), [childItems]);
  const candidatesQuery = useQuery({
    queryFn: () => listGroupChildCandidates(currentIndexId, null),
    queryKey: groupChildCandidatesQueryKey(currentIndexId),
  });
  const candidates = useMemo(() => candidatesQuery.data ?? [], [candidatesQuery.data]);
  const candidatesById = useMemo(
    () => new Map(candidates.map((candidate) => [candidate.profileId, candidate])),
    [candidates],
  );
  const isProxyChain = configType === CONFIG_TYPES.ProxyChain;
  const groupType = isProxyChain ? "ProxyChain" : "PolicyGroup";
  const [pickerDraftIds, setPickerDraftIds] = useState<string[]>(selectedIds);

  useEffect(() => {
    return () => {
      previewRequestId.current += 1;
    };
  }, []);

  function setSelectedIds(ids: string[]) {
    setValue("protocolOptions.childProfileIds", ids.join(","), {
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
    const requestId = previewRequestId.current + 1;
    previewRequestId.current = requestId;
    setPreviewLoading(true);
    setPreviewError(null);
    try {
      const draft = prepareGroupDraftForPreview(getValues(), t("panes.groups.draftName"));
      const nextPreview = await previewGroupProfile(draft);
      if (!mountedRef.current || requestId !== previewRequestId.current) {
        return;
      }
      setPreview(nextPreview);
    } catch (error) {
      if (!mountedRef.current || requestId !== previewRequestId.current) {
        return;
      }
      setPreviewError(getErrorMessage(error));
    } finally {
      if (mountedRef.current && requestId === previewRequestId.current) {
        setPreviewLoading(false);
      }
    }
  }

  return (
    <div className="grid gap-4">
      <div className="grid gap-3 lg:grid-cols-[1fr_1fr_10rem]">
        <GroupTypeField
          groupType={groupType}
          label={t(isProxyChain ? "panes.groups.chainMarker" : "panes.groups.groupMarker")}
        />
        <LabeledField
          label={t("panes.groups.subscriptionChildGroup")}
          {...register("protocolOptions.sourceSubscriptionId")}
        />
        <div className="grid gap-1.5">
          <Label htmlFor={multipleLoadId}>{t("panes.groups.loadMode")}</Label>
          <Select
            onValueChange={(value) => {
              setValue("protocolOptions.loadStrategy", value as LoadStrategy, {
                shouldDirty: true,
                shouldValidate: true,
              });
            }}
            value={multipleLoad ?? "leastPing"}
          >
            <SelectTrigger className="w-full" id={multipleLoadId}>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {loadStrategyOptions(t).map((option) => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      <div className="grid gap-3 lg:grid-cols-[1fr_13rem]">
        <LabeledField
          label={t("panes.groups.subscriptionFilter")}
          placeholder="^US|Japan"
          {...register("protocolOptions.filter")}
        />
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
            {t("panes.groups.chooseChildren")}
          </Button>
        </div>
      </div>

      <div className="grid gap-2">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2 text-sm font-medium">
            <Layers3 className="size-4 text-muted-foreground" aria-hidden="true" />
            {t("panes.groups.selectedChildren")}
          </div>
          <Button disabled={previewLoading} onClick={() => void loadPreview()} size="sm" type="button" variant="outline">
            <Eye className="size-4" aria-hidden="true" />
            {t("panes.groups.preview")}
          </Button>
        </div>

        {selectedIds.length === 0 ? (
          <div className="rounded-lg border border-dashed bg-card px-3 py-6 text-center text-sm text-muted-foreground">
            {t("panes.groups.noChildren")}
          </div>
        ) : (
          <div className="grid gap-2">
            {selectedIds.map((indexId, index) => {
              const candidate = candidatesById.get(indexId);

              return (
                <div className="grid grid-cols-[1.5rem_1fr_auto] items-center gap-2 rounded-lg border bg-card px-3 py-2" key={indexId}>
                  <span className="text-xs tabular-nums text-muted-foreground">{index + 1}</span>
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium">
                      {candidate?.remarks || indexId}
                    </div>
                    <div className="truncate text-xs text-muted-foreground">
                      {candidate
                        ? `${getProtocolLabel(candidate.protocol)} · ${candidate.address || t("panes.groups.groupFallback")}`
                        : indexId}
                    </div>
                  </div>
                  <div className="flex items-center gap-1">
                    <IconButton
                      disabled={index === 0}
                      label={t("panes.groups.moveChildUp")}
                      onClick={() => moveChild(indexId, -1)}
                    >
                      <ArrowUp className="size-4" aria-hidden="true" />
                    </IconButton>
                    <IconButton
                      disabled={index + 1 === selectedIds.length}
                      label={t("panes.groups.moveChildDown")}
                      onClick={() => moveChild(indexId, 1)}
                    >
                      <ArrowDown className="size-4" aria-hidden="true" />
                    </IconButton>
                    <IconButton label={t("panes.groups.removeChild")} onClick={() => removeChild(indexId)}>
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
  const { t } = useI18n();
  const [filter, setFilter] = useState("");
  const searchId = useId();
  const pickerId = useId();
  const draftIdSet = useMemo(() => new Set(draftIds), [draftIds]);
  const filtered = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    if (!needle) {
      return candidates;
    }

    return candidates.filter((candidate) =>
      [candidate.remarks, candidate.address, candidate.profileId, getProtocolLabel(candidate.protocol)]
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
        return current.includes(candidate.profileId) ? current : [...current, candidate.profileId];
      }

      return current.filter((indexId) => indexId !== candidate.profileId);
    });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <ScrollableDialogContent
        closeLabel={t("actions.close")}
        height="compact"
        rows="toolbar-body"
        width="54rem"
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Layers3 className="size-4" aria-hidden="true" />
            {t("panes.groups.selectChildren")}
          </DialogTitle>
          <DialogDescription className="sr-only">
            {t("panes.groups.chooseDescription")}
          </DialogDescription>
        </DialogHeader>

        <div className="grid gap-1.5">
          <Label className="sr-only" htmlFor={searchId}>
            {t("panes.groups.filterChildren")}
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
              aria-label={t("panes.groups.filterChildren")}
              onChange={(event) => setFilter(event.target.value)}
              placeholder={t("panes.groups.filterChildren")}
              value={filter}
            />
          </div>
        </div>

        <ScrollArea className="min-h-0 overflow-hidden rounded-md border" id={pickerId}>
          {loading ? (
            <div className="grid h-40 place-items-center text-sm text-muted-foreground">{t("panes.groups.loadingChildren")}</div>
          ) : filtered.length === 0 ? (
            <div className="grid h-40 place-items-center text-sm text-muted-foreground">{t("panes.groups.noMatches")}</div>
          ) : (
            <div className="divide-y">
              {filtered.map((candidate) => {
                const checked = draftIdSet.has(candidate.profileId);
                const checkboxId = `${pickerId}-${toDomId(candidate.profileId)}`;

                return (
                  <Label
                    className={cn(
                      "grid cursor-default grid-cols-[1.25rem_minmax(0,1fr)_auto] items-center gap-3 px-3 py-2 text-sm leading-normal",
                      candidate.selectable ? "hover:bg-accent" : "text-muted-foreground",
                    )}
                    htmlFor={checkboxId}
                    key={candidate.profileId}
                  >
                    <Checkbox
                      checked={checked}
                      disabled={!candidate.selectable}
                      id={checkboxId}
                      onCheckedChange={(nextChecked) => toggleCandidate(candidate, nextChecked === true)}
                    />
                    <span className="min-w-0">
                      <span className="block truncate font-medium">{candidate.remarks || candidate.profileId}</span>
                      <span className="block truncate text-xs text-muted-foreground">
                        {candidate.profileId} · {candidate.address || t("panes.groups.groupFallback")}
                      </span>
                    </span>
                    <Badge className="justify-self-end bg-background text-muted-foreground" variant="outline">
                      {candidate.isGroup ? t("panes.groups.nested") : getProtocolLabel(candidate.protocol)}
                    </Badge>
                  </Label>
                );
              })}
            </div>
          )}
        </ScrollArea>

        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} type="button" variant="outline">
            {t("actions.cancel")}
          </Button>
          <Button
            onClick={() => {
              onSelected(draftIds);
              onOpenChange(false);
            }}
            type="button"
          >
            {t("actions.apply")}
          </Button>
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}

function GroupPreviewPanel({ preview }: { preview: GroupPreview }) {
  const { t } = useI18n();
  const validation = preview.validation;
  const singboxRoutes = preview.singboxRoutes;

  return (
    <Card className="grid gap-3 rounded-lg bg-muted/30 p-3 shadow-none">
      {validation.valid ? (
        <div className="flex items-center gap-2 text-sm text-success">
          <CheckCircle2 className="size-4" aria-hidden="true" />
          {t("panes.groups.previewGenerated")}
        </div>
      ) : (
        <ValidationMessage tone="error" messages={validation.errors ?? []} />
      )}
      {(validation.warnings ?? []).length > 0 ? (
        <ValidationMessage tone="warning" messages={validation.warnings ?? []} />
      ) : null}

      <div className="grid gap-3">
        <PreviewList routes={singboxRoutes} title={t("panes.groups.previewRoutesTitle")} />
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
  const { t } = useI18n();
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
          <div className="px-3 py-6 text-center text-sm text-muted-foreground">{t("panes.groups.noGeneratedRoutes")}</div>
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
                  {route.dialerProxy
                    ? t("panes.groups.routeDialerProxy", { value: route.dialerProxy })
                    : null}
                  {route.detour ? t("panes.groups.routeDetour", { value: route.detour }) : null}
                  {route.outbounds.length > 0
                    ? t("panes.groups.routeOutbounds", { value: route.outbounds.join(", ") })
                    : null}
                  {route.downloadDialerProxy
                    ? t("panes.groups.routeDownloadDialerProxy", { value: route.downloadDialerProxy })
                    : null}
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
  const { t } = useI18n();
  if (messages.length === 0) {
    return null;
  }

  return (
    <Alert
      className={cn(
        "gap-1 px-3 py-2",
        tone === "error" ? "border-destructive/40 bg-destructive/10 text-destructive" : "border-warning-bold/40 bg-warning-bg text-warning",
      )}
      role={tone === "error" ? "alert" : "status"}
      variant={tone === "error" ? "destructive" : "default"}
    >
      <AlertTriangle className="size-4" aria-hidden="true" />
      <AlertTitle>
        {tone === "error" ? t("panes.groups.validationFailed") : t("panes.groups.validationWarnings")}
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

function GroupTypeField({
  groupType,
  label,
}: {
  groupType: "PolicyGroup" | "ProxyChain";
  label: string;
}) {
  const inputId = useId();

  return (
    <div className="grid gap-1.5">
      <Label htmlFor={inputId}>{label}</Label>
      <Input id={inputId} readOnly value={groupType} />
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
  const ids = new Set<string>();

  for (const item of (value ?? "").split(",")) {
    const indexId = item.trim();
    if (indexId) {
      ids.add(indexId);
    }
  }

  return [...ids];
}

function toDomId(value: string) {
  return value.replace(/[^A-Za-z0-9_-]/g, "_");
}
