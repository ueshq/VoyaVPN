import { useState } from "react";
import type * as React from "react";
import { Plus, RefreshCw, Rss, Save, Trash2 } from "lucide-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { EmptyState } from "@/components/ui/empty-state";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import {
  deleteSubscriptions,
  listSubscriptions,
  saveSubscription,
  updateSubscriptions,
} from "@/ipc";
import type { SubItem_Deserialize } from "@/ipc/bindings";
import { useI18n } from "@/i18n/use-i18n";
import { redactOperationalError } from "@/lib/operational-redaction";
import { cn } from "@/lib/utils";

type SubscriptionsDialogProps = {
  onChanged: () => void;
  onOpenChange: (open: boolean) => void;
  open: boolean;
};

function createBlankSubscription(): SubItem_Deserialize {
  return {
    AutoUpdateInterval: 0,
    Enabled: true,
    Filter: null,
    MoreUrl: "",
    Remarks: "",
    Url: "",
    UserAgent: "",
  };
}

export function SubscriptionsDialog({ onChanged, onOpenChange, open }: SubscriptionsDialogProps) {
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState<SubItem_Deserialize>(() => createBlankSubscription());
  const [message, setMessage] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState("");
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const subscriptionsQuery = useQuery({
    enabled: open,
    queryFn: listSubscriptions,
    queryKey: ["subscriptions"],
  });
  const subscriptions = subscriptionsQuery.data ?? [];

  async function refreshSubscriptions() {
    await queryClient.invalidateQueries({ queryKey: ["subscriptions"] });
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
      const saved = await saveSubscription(form);
      setSelectedId(saved.Id);
      setForm(saved);

      return "Subscription saved";
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

      return "Subscription deleted";
    });
  }

  async function handleUpdate(id: string | null) {
    await run(async () => {
      const result = await updateSubscriptions(id, false, null);

      return `${result.updated ?? 0} updated, ${result.imported ?? 0} profiles imported`;
    });
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[92vh] max-w-5xl grid-rows-[auto,minmax(0,1fr),auto] overflow-hidden">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Rss className="size-4" aria-hidden="true" />
            Subscriptions
          </DialogTitle>
          <DialogDescription className="sr-only">Manage subscription sources and run updates.</DialogDescription>
        </DialogHeader>

        <div className="grid min-h-0 gap-4 lg:grid-cols-[18rem_1fr]">
          <Card className="min-h-0 gap-0 rounded-md bg-background p-0 shadow-none">
            <CardHeader className="flex h-10 flex-row items-center justify-between border-b px-3 py-0">
              <CardTitle className="text-sm">Sources</CardTitle>
              <Button
                aria-label="New subscription"
                className="size-7"
                onClick={() => {
                  setSelectedId("");
                  setForm(createBlankSubscription());
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
                          selectedId === item.Id ? "bg-accent text-accent-foreground" : null,
                        )}
                        key={item.Id}
                        onClick={() => {
                          setSelectedId(item.Id);
                          setForm(item);
                        }}
                        type="button"
                      >
                        <span className="truncate font-medium">{item.Remarks || "Untitled"}</span>
                        <Badge className="self-start" variant={item.Enabled ? "secondary" : "outline"}>
                          {item.Enabled ? "Enabled" : "Disabled"}
                        </Badge>
                        <span className="col-span-2 truncate text-xs text-muted-foreground">{item.Url}</span>
                      </button>
                    ))
                  )}
                </div>
              </ScrollArea>
            </CardContent>
          </Card>

          <Card className="min-h-0 gap-3 rounded-md bg-background p-3 shadow-none">
            <CardHeader className="p-0">
              <CardTitle className="text-sm">Subscription details</CardTitle>
            </CardHeader>
            <CardContent className="grid content-start gap-3 p-0">
              <div className="grid gap-3 md:grid-cols-2">
                <TextField
                  label="Remarks"
                  onChange={(value) => setForm((current) => ({ ...current, Remarks: value }))}
                  value={form.Remarks ?? ""}
                />
                <TextField
                  label="User agent"
                  onChange={(value) => setForm((current) => ({ ...current, UserAgent: value }))}
                  value={form.UserAgent ?? ""}
                />
              </div>
              <TextField
                label="URL"
                onChange={(value) => setForm((current) => ({ ...current, Url: value }))}
                value={form.Url ?? ""}
              />
              <TextField
                label="More URL"
                onChange={(value) => setForm((current) => ({ ...current, MoreUrl: value }))}
                value={form.MoreUrl ?? ""}
              />
              <div className="grid gap-3 md:grid-cols-3">
                <TextField
                  label="Filter"
                  onChange={(value) => setForm((current) => ({ ...current, Filter: value || null }))}
                  value={form.Filter ?? ""}
                />
                <TextField
                  label="Convert target"
                  onChange={(value) => setForm((current) => ({ ...current, ConvertTarget: value || null }))}
                  value={form.ConvertTarget ?? ""}
                />
                <TextField
                  label="Auto update minutes"
                  onChange={(value) => setForm((current) => ({ ...current, AutoUpdateInterval: Number(value) || 0 }))}
                  type="number"
                  value={String(form.AutoUpdateInterval ?? 0)}
                />
              </div>
              <div className="flex h-9 items-center rounded-md border bg-card px-3 shadow-xs">
                <Label
                  className="h-full w-fit cursor-pointer text-xs font-medium text-muted-foreground"
                  htmlFor="subscription-enabled"
                >
                  <Checkbox
                    checked={form.Enabled ?? true}
                    id="subscription-enabled"
                    onCheckedChange={(checked) => setForm((current) => ({ ...current, Enabled: checked === true }))}
                  />
                  Enabled
                </Label>
              </div>

              <div className="flex flex-wrap gap-2">
                <Button onClick={() => void handleSave()} type="button">
                  <Save className="size-4" aria-hidden="true" />
                  Save
                </Button>
                <Button disabled={!selectedId} onClick={() => void handleDelete()} type="button" variant="outline">
                  <Trash2 className="size-4" aria-hidden="true" />
                  Delete
                </Button>
                <Button disabled={!selectedId} onClick={() => void handleUpdate(selectedId)} type="button" variant="outline">
                  <RefreshCw className="size-4" aria-hidden="true" />
                  Update selected
                </Button>
                <Button onClick={() => void handleUpdate(null)} type="button" variant="outline">
                  <RefreshCw className="size-4" aria-hidden="true" />
                  Update all
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
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
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
