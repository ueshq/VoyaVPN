import { useState } from "react";
import type * as React from "react";
import { Plus, RefreshCw, Rss, Save, Trash2 } from "lucide-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { Card, CardContent, CardHeader, CardTitle } from "@voya/ui/components/card";
import { Checkbox } from "@voya/ui/components/checkbox";
import {
  Dialog,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { EmptyState } from "@voya/ui/components/empty-state";
import { Input } from "@voya/ui/components/input";
import { Label } from "@voya/ui/components/label";
import { ScrollArea } from "@voya/ui/components/scroll-area";
import { Skeleton } from "@voya/ui/components/skeleton";
import {
  deleteSubscriptions,
  listSubscriptionMetadata,
  listSubscriptions,
  saveSubscription,
  updateSubscriptions,
} from "@/ipc";
import type { Subscription } from "@/ipc/bindings";
import { useI18n } from "@voya/i18n/use-i18n";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import { cn } from "@voya/ui/lib/utils";

import { SubscriptionMetaLine } from "./subscription-card";
import { metadataBySubscriptionId } from "./subscription-usage";

type SubscriptionsDialogProps = {
  onChanged: () => void;
  onOpenChange: (open: boolean) => void;
  open: boolean;
};

function createBlankSubscription(): Subscription {
  return {
    additionalUrl: "",
    autoUpdateIntervalMinutes: null,
    converterTarget: null,
    enabled: true,
    filter: null,
    id: "",
    preSocksPort: null,
    remarks: "",
    sort: 0,
    url: "",
    userAgent: "",
  };
}

/** UI edits the interval in hours; the DTO stores minutes (0/empty = off). */
function intervalHoursFromMinutes(minutes: number | null): string {
  if (minutes == null || minutes <= 0) {
    return "";
  }
  return String(Math.round((minutes / 60) * 10) / 10);
}

function intervalMinutesFromHours(hours: string): number | null {
  const parsed = Number.parseFloat(hours);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return null;
  }
  return Math.max(1, Math.round(parsed * 60));
}

export function SubscriptionsDialog({ onChanged, onOpenChange, open }: SubscriptionsDialogProps) {
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState<Subscription>(() => createBlankSubscription());
  const [intervalHours, setIntervalHours] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState("");
  const { language, t } = useI18n();
  const queryClient = useQueryClient();
  const subscriptionsQuery = useQuery({
    enabled: open,
    queryFn: listSubscriptions,
    queryKey: ["subscriptions"],
  });
  const metadataQuery = useQuery({
    enabled: open,
    queryFn: listSubscriptionMetadata,
    queryKey: ["subscription-metadata"],
  });
  const subscriptions = subscriptionsQuery.data ?? [];
  const metadataById = metadataBySubscriptionId(metadataQuery.data ?? []);

  async function refreshSubscriptions() {
    await queryClient.invalidateQueries({ queryKey: ["subscriptions"] });
    await queryClient.invalidateQueries({ queryKey: ["subscription-metadata"] });
    onChanged();
  }

  async function run(operation: () => Promise<string | null>) {
    setError(null);
    setMessage(null);
    try {
      const nextMessage = await operation();
      if (nextMessage) {
        setMessage(nextMessage);
      }
      await refreshSubscriptions();
    } catch (error) {
      setError(redactOperationalError(error));
    }
  }

  async function handleSave() {
    await run(async () => {
      const saved = await saveSubscription({
        ...form,
        autoUpdateIntervalMinutes: intervalMinutesFromHours(intervalHours),
      });
      setSelectedId(saved.id);
      setForm(saved);
      setIntervalHours(intervalHoursFromMinutes(saved.autoUpdateIntervalMinutes));

      return t("panes.subscriptions.saved");
    });
  }

  async function handleDelete() {
    if (!selectedId) {
      return;
    }
    await run(async () => {
      await deleteSubscriptions([selectedId]);
      setSelectedId("");
      setForm(createBlankSubscription());
      setIntervalHours("");

      return t("panes.subscriptions.deleted");
    });
  }

  async function handleUpdate(id: string | null) {
    await run(async () => {
      const result = await updateSubscriptions(id, true, null);

      return t("panes.subscriptions.updateResult", {
        imported: result.imported ?? 0,
        updated: result.updated ?? 0,
      });
    });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <ScrollableDialogContent closeLabel={t("actions.close")} width="5xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Rss className="size-4" aria-hidden="true" />
            {t("panes.subscriptions.title")}
          </DialogTitle>
          <DialogDescription className="sr-only">{t("panes.subscriptions.description")}</DialogDescription>
        </DialogHeader>

        <div className="grid min-h-0 gap-4 lg:grid-cols-[18rem_1fr]">
          <Card className="min-h-0 gap-0 rounded-xl bg-surface-raised p-0 shadow-raised">
            <CardHeader className="flex h-10 flex-row items-center justify-between border-b px-3 py-0">
              <CardTitle className="text-xs uppercase tracking-wide text-muted-foreground">{t("panes.subscriptions.sources")}</CardTitle>
              <Button
                aria-label={t("panes.subscriptions.new")}
                className="size-7"
                onClick={() => {
                  setSelectedId("");
                  setForm(createBlankSubscription());
                  setIntervalHours("");
                }}
                size="icon"
                type="button"
                variant="ghost"
              >
                <Plus className="size-4" aria-hidden="true" />
              </Button>
            </CardHeader>
            <CardContent className="min-h-0 p-0">
              <ScrollArea className="h-[24rem]">
                <div className="p-1">
                  {subscriptionsQuery.isLoading ? (
                    <SubscriptionSkeletonRows aria-label={t("panes.subscriptions.loading")} />
                  ) : subscriptions.length === 0 ? (
                    <EmptyState
                      description={t("panes.subscriptions.emptyDescription")}
                      icon={Rss}
                      title={t("panes.subscriptions.empty")}
                    />
                  ) : (
                    subscriptions.map((item) => (
                      <button
                        className={cn(
                          "grid w-full grid-cols-[minmax(0,1fr)_auto] gap-x-2 gap-y-1 rounded-md px-2 py-2 text-start text-sm outline-none transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50",
                          selectedId === item.id ? "bg-accent text-accent-foreground" : null,
                        )}
                        key={item.id}
                        onClick={() => {
                          setSelectedId(item.id);
                          setForm(item);
                          setIntervalHours(intervalHoursFromMinutes(item.autoUpdateIntervalMinutes));
                        }}
                        type="button"
                      >
                        <span className="truncate font-medium">{item.remarks || t("panes.subscriptions.untitled")}</span>
                        <Badge className="self-start" variant={item.enabled ? "secondary" : "outline"}>
                          {item.enabled ? t("panes.subscriptions.enabled") : t("panes.subscriptions.disabled")}
                        </Badge>
                        <span className="col-span-2 truncate text-xs text-muted-foreground">{item.url}</span>
                        <SubscriptionMetaLine
                          className="col-span-2"
                          language={language}
                          metadata={metadataById.get(item.id)}
                          t={t}
                        />
                      </button>
                    ))
                  )}
                </div>
              </ScrollArea>
            </CardContent>
          </Card>

          <Card className="min-h-0 gap-3 rounded-xl bg-surface-raised p-3 shadow-raised">
            <CardHeader className="p-0">
              <CardTitle className="text-xs uppercase tracking-wide text-muted-foreground">{t("panes.subscriptions.details")}</CardTitle>
            </CardHeader>
            <CardContent className="grid content-start gap-3 p-0">
              <div className="grid gap-3 md:grid-cols-2">
                <TextField
                  label={t("panes.subscriptions.remarks")}
                  onChange={(value) => setForm((current) => ({ ...current, remarks: value }))}
                  value={form.remarks}
                />
                <TextField
                  label={t("panes.subscriptions.userAgent")}
                  onChange={(value) => setForm((current) => ({ ...current, userAgent: value }))}
                  value={form.userAgent}
                />
              </div>
              <TextField
                label={t("panes.subscriptions.url")}
                onChange={(value) => setForm((current) => ({ ...current, url: value }))}
                value={form.url}
              />
              <TextField
                label={t("panes.subscriptions.additionalUrl")}
                onChange={(value) => setForm((current) => ({ ...current, additionalUrl: value }))}
                value={form.additionalUrl}
              />
              <div className="grid gap-3 md:grid-cols-2">
                <TextField
                  label={t("panes.subscriptions.filter")}
                  onChange={(value) => setForm((current) => ({ ...current, filter: value || null }))}
                  value={form.filter ?? ""}
                />
                <TextField
                  label={t("panes.subscriptions.convertTarget")}
                  onChange={(value) => setForm((current) => ({ ...current, converterTarget: value || null }))}
                  value={form.converterTarget ?? ""}
                />
              </div>
              <div className="grid gap-1">
                <TextField
                  label={t("panes.subscriptions.autoUpdateInterval")}
                  min={0}
                  onChange={setIntervalHours}
                  step="0.5"
                  type="number"
                  value={intervalHours}
                />
                <p className="text-xs text-muted-foreground">{t("panes.subscriptions.autoUpdateHint")}</p>
              </div>
              <div className="flex h-9 items-center rounded-md border bg-card px-3 shadow-xs">
                <Label
                  className="h-full w-fit cursor-pointer text-xs font-medium text-muted-foreground"
                  htmlFor="subscription-enabled"
                >
                  <Checkbox
                    checked={form.enabled}
                    id="subscription-enabled"
                    onCheckedChange={(checked) => setForm((current) => ({ ...current, enabled: checked === true }))}
                  />
                  {t("panes.subscriptions.enabled")}
                </Label>
              </div>

              <div className="flex flex-wrap gap-2">
                <Button onClick={() => void handleSave()} type="button">
                  <Save className="size-4" aria-hidden="true" />
                  {t("actions.save")}
                </Button>
                <Button disabled={!selectedId} onClick={() => void handleDelete()} type="button" variant="outline">
                  <Trash2 className="size-4" aria-hidden="true" />
                  {t("actions.delete")}
                </Button>
                <Button disabled={!selectedId} onClick={() => void handleUpdate(selectedId)} type="button" variant="outline">
                  <RefreshCw className="size-4" aria-hidden="true" />
                  {t("panes.subscriptions.updateSelected")}
                </Button>
                <Button onClick={() => void handleUpdate(null)} type="button" variant="outline">
                  <RefreshCw className="size-4" aria-hidden="true" />
                  {t("panes.subscriptions.updateAll")}
                </Button>
              </div>

              {message ? (
                <Alert role="status">
                  <AlertDescription>{message}</AlertDescription>
                </Alert>
              ) : null}
              {error ? (
                <Alert variant="destructive">
                  <AlertDescription>{error}</AlertDescription>
                </Alert>
              ) : null}
            </CardContent>
          </Card>
        </div>

        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} type="button" variant="outline">
            {t("actions.close")}
          </Button>
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}

// Mirror the source button layout (remarks + status badge + url) so the loading
// state matches the populated list — the connections pane skeleton pattern.
function SubscriptionSkeletonRows(props: React.ComponentProps<"div">) {
  return (
    <div role="status" {...props}>
      {Array.from({ length: 5 }).map((_, index) => (
        <div className="grid grid-cols-[minmax(0,1fr)_auto] gap-x-2 gap-y-1 px-2 py-2" key={index}>
          <Skeleton className="h-4 w-2/3" />
          <Skeleton className="h-5 w-16 rounded-full" />
          <Skeleton className="col-span-2 h-3 w-4/5" />
        </div>
      ))}
    </div>
  );
}

function TextField({
  className,
  id,
  label,
  onChange,
  type = "text",
  value,
  ...props
}: Omit<React.ComponentProps<typeof Input>, "onChange" | "value"> & {
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  const inputId = id ?? `subscription-${fieldId(label)}`;

  return (
    <div className="grid min-w-0 gap-1">
      <Label className="text-xs text-muted-foreground" htmlFor={inputId}>
        <span className="truncate">{label}</span>
      </Label>
      <Input
        className={cn("bg-card", className)}
        id={inputId}
        onChange={(event) => onChange(event.target.value)}
        type={type}
        value={value}
        {...props}
      />
    </div>
  );
}

function fieldId(label: string) {
  return label.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-").replaceAll(/^-|-$/g, "");
}
