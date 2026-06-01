import { useState } from "react";
import { Plus, RefreshCw, Rss, Save, Trash2 } from "lucide-react";
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
  deleteSubscriptions,
  listSubscriptions,
  saveSubscription,
  updateSubscriptions,
} from "@/ipc";
import type { SubItem_Deserialize } from "@/ipc/bindings";
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
      setError(error instanceof Error ? error.message : String(error));
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
      <DialogContent className="max-w-5xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Rss className="size-4" aria-hidden="true" />
            Subscriptions
          </DialogTitle>
          <DialogDescription className="sr-only">Manage subscription sources and run updates.</DialogDescription>
        </DialogHeader>

        <div className="grid min-h-[28rem] gap-4 lg:grid-cols-[18rem_1fr]">
          <div className="min-h-0 overflow-hidden rounded-md border">
            <div className="flex h-10 items-center justify-between border-b px-3">
              <span className="text-sm font-medium">Sources</span>
              <Button
                aria-label="New subscription"
                className="size-7 p-0"
                onClick={() => {
                  setSelectedId("");
                  setForm(createBlankSubscription());
                }}
                type="button"
                variant="ghost"
              >
                <Plus className="size-4" aria-hidden="true" />
              </Button>
            </div>
            <div className="max-h-[24rem] overflow-auto p-1">
              {subscriptions.length === 0 ? (
                <p className="px-2 py-3 text-sm text-muted-foreground">No subscriptions</p>
              ) : (
                subscriptions.map((item) => (
                  <button
                    className={cn(
                      "grid w-full gap-1 rounded-sm px-2 py-2 text-start text-sm outline-none hover:bg-accent",
                      selectedId === item.Id ? "bg-accent" : null,
                    )}
                    key={item.Id}
                    onClick={() => {
                      setSelectedId(item.Id);
                      setForm(item);
                    }}
                    type="button"
                  >
                    <span className="truncate font-medium">{item.Remarks || "Untitled"}</span>
                    <span className="truncate text-xs text-muted-foreground">{item.Url}</span>
                  </button>
                ))
              )}
            </div>
          </div>

          <div className="grid content-start gap-3">
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
            <label className="flex items-center gap-2 text-sm">
              <input
                checked={form.Enabled ?? true}
                className="size-4 accent-primary"
                onChange={(event) => setForm((current) => ({ ...current, Enabled: event.target.checked }))}
                type="checkbox"
              />
              Enabled
            </label>

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

            {message ? <p className="text-sm text-muted-foreground">{message}</p> : null}
            {error ? (
              <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">
                {error}
              </p>
            ) : null}
          </div>
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

function TextField({
  label,
  onChange,
  type = "text",
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  type?: "number" | "text";
  value: string;
}) {
  return (
    <label className="grid gap-1 text-sm">
      <span className="text-xs font-medium text-muted-foreground">{label}</span>
      <input
        className="h-9 rounded-md border bg-background px-2 outline-none focus:ring-2 focus:ring-ring"
        onChange={(event) => onChange(event.target.value)}
        type={type}
        value={value}
      />
    </label>
  );
}
