import { useMemo, useState } from "react";
import { ClipboardPaste, FileUp, Upload } from "lucide-react";
import { useQuery } from "@tanstack/react-query";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { importProfilesFromText, listSubscriptions } from "@/ipc";

type ImportProfilesDialogProps = {
  onImported: () => void;
  onOpenChange: (open: boolean) => void;
  open: boolean;
};

export function ImportProfilesDialog({ onImported, onOpenChange, open }: ImportProfilesDialogProps) {
  const [error, setError] = useState<string | null>(null);
  const [resultText, setResultText] = useState<string | null>(null);
  const [selectedSubid, setSelectedSubid] = useState("");
  const [text, setText] = useState("");
  const subscriptionsQuery = useQuery({
    enabled: open,
    queryFn: listSubscriptions,
    queryKey: ["subscriptions"],
  });
  const subscriptions = useMemo(() => subscriptionsQuery.data ?? [], [subscriptionsQuery.data]);
  const canImport = text.trim().length > 0;
  const targetLabel = useMemo(() => {
    const selected = subscriptions.find((item) => item.Id === selectedSubid);

    return selected ? selected.Remarks : "Manual import";
  }, [selectedSubid, subscriptions]);

  async function handleImport() {
    if (!canImport) {
      return;
    }
    setError(null);
    setResultText(null);
    try {
      const result = await importProfilesFromText(text, selectedSubid || null, Boolean(selectedSubid));
      setResultText(
        `${result.imported ?? 0} imported, ${result.skipped ?? 0} skipped for ${targetLabel}`,
      );
      onImported();
      onOpenChange(false);
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    }
  }

  async function handlePaste() {
    if (!navigator.clipboard?.readText) {
      setError("Clipboard read is unavailable in this context.");
      return;
    }
    setError(null);
    setText(await navigator.clipboard.readText());
  }

  async function handleFile(file: File | null) {
    if (!file) {
      return;
    }
    setError(null);
    setText(await file.text());
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Upload className="size-4" aria-hidden="true" />
            Import Profiles
          </DialogTitle>
          <DialogDescription className="sr-only">Import share links, subscription URLs, or JSON payloads.</DialogDescription>
        </DialogHeader>

        <div className="grid gap-3">
          <div className="flex flex-wrap items-center gap-2">
            <label className="grid min-w-56 gap-1 text-sm">
              <span className="text-xs font-medium text-muted-foreground">Target</span>
              <select
                className="h-9 rounded-md border bg-background px-2"
                onChange={(event) => setSelectedSubid(event.target.value)}
                value={selectedSubid}
              >
                <option value="">Manual import</option>
                {subscriptions.map((item) => (
                  <option key={item.Id} value={item.Id}>
                    {item.Remarks || item.Url}
                  </option>
                ))}
              </select>
            </label>

            <Button className="mt-5" onClick={() => void handlePaste()} type="button" variant="outline">
              <ClipboardPaste className="size-4" aria-hidden="true" />
              Paste
            </Button>

            <label className="mt-5 inline-flex h-9 cursor-pointer items-center gap-2 rounded-md border px-3 text-sm">
              <FileUp className="size-4" aria-hidden="true" />
              File
              <input
                className="sr-only"
                onChange={(event) => void handleFile(event.target.files?.[0] ?? null)}
                type="file"
              />
            </label>
          </div>

          <label className="grid gap-1 text-sm">
            <span className="text-xs font-medium text-muted-foreground">Import payload</span>
            <textarea
              className="min-h-72 resize-y rounded-md border bg-background p-3 font-mono text-xs outline-none focus:ring-2 focus:ring-ring"
              onChange={(event) => setText(event.target.value)}
              value={text}
            />
          </label>

          {resultText ? <p className="text-sm text-muted-foreground">{resultText}</p> : null}
          {error ? (
            <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">
              {error}
            </p>
          ) : null}
        </div>

        <DialogFooter>
          <Button onClick={() => onOpenChange(false)} type="button" variant="outline">
            Close
          </Button>
          <Button disabled={!canImport} onClick={() => void handleImport()} type="button">
            Import payload
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
