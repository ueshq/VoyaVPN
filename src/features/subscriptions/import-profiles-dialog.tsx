import { useMemo, useState } from "react";
import { ClipboardPaste, FileUp, Upload } from "lucide-react";
import { useQuery } from "@tanstack/react-query";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { importProfilesFromText, listSubscriptions } from "@/ipc";
import { redactOperationalError } from "@/lib/operational-redaction";

type ImportProfilesDialogProps = {
  onImported: () => void;
  onOpenChange: (open: boolean) => void;
  open: boolean;
};

const EMPTY_SELECT_VALUE = "__voyavpn_manual_import__";

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
      setText("");
      onImported();
    } catch (error) {
      setError(redactOperationalError(error));
    }
  }

  async function handlePaste() {
    if (!navigator.clipboard?.readText) {
      setError("Clipboard read is unavailable in this context.");
      return;
    }
    setError(null);
    try {
      setText(await navigator.clipboard.readText());
    } catch (error) {
      setError(redactOperationalError(error));
    }
  }

  async function handleFile(file: File | null) {
    if (!file) {
      return;
    }
    setError(null);
    try {
      setText(await file.text());
    } catch (error) {
      setError(redactOperationalError(error));
    }
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

        <Card className="gap-3 rounded-xl bg-surface-raised p-3 shadow-raised">
          <CardContent className="grid gap-3 p-0">
            <div className="grid gap-3 md:grid-cols-[minmax(14rem,1fr)_12rem_auto_auto] md:items-end">
              <div className="grid min-w-0 gap-1">
                <Label className="text-xs text-muted-foreground" htmlFor="import-target">
                  Target
                </Label>
                <Select
                  onValueChange={(value) => setSelectedSubid(decodeSelectValue(value))}
                  value={encodeSelectValue(selectedSubid)}
                >
                  <SelectTrigger className="w-full bg-card" id="import-target">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value={EMPTY_SELECT_VALUE}>Manual import</SelectItem>
                    {subscriptions.map((item) => (
                      <SelectItem key={item.Id} value={item.Id}>
                        {item.Remarks || item.Url}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="grid min-w-0 gap-1">
                <Label className="text-xs text-muted-foreground" htmlFor="import-subscription-target">
                  Mode
                </Label>
                <div className="flex h-9 items-center rounded-md border bg-card px-3 shadow-xs">
                  <Label
                    className="h-full min-w-0 cursor-pointer text-xs font-medium text-muted-foreground"
                    htmlFor="import-subscription-target"
                  >
                    <Checkbox
                      checked={Boolean(selectedSubid)}
                      disabled={subscriptions.length === 0}
                      id="import-subscription-target"
                      onCheckedChange={(checked) => {
                        if (checked === true) {
                          setSelectedSubid((current) => current || subscriptions[0]?.Id || "");
                          return;
                        }

                        setSelectedSubid("");
                      }}
                    />
                    <span className="truncate">Subscription target</span>
                  </Label>
                </div>
              </div>

              <Button onClick={() => void handlePaste()} type="button" variant="outline">
                <ClipboardPaste className="size-4" aria-hidden="true" />
                Paste
              </Button>

              <Button asChild variant="outline">
                <Label className="cursor-pointer" htmlFor="import-payload-file">
                  <FileUp className="size-4" aria-hidden="true" />
                  File
                </Label>
              </Button>
              <input
                className="sr-only"
                id="import-payload-file"
                onChange={(event) => void handleFile(event.target.files?.[0] ?? null)}
                type="file"
              />
            </div>

            <div className="grid gap-1">
              <Label className="text-xs text-muted-foreground" htmlFor="import-payload">
                Import payload
              </Label>
              <Textarea
                className="min-h-72 resize-y bg-card font-mono text-xs"
                id="import-payload"
                onChange={(event) => {
                  setResultText(null);
                  setText(event.target.value);
                }}
                value={text}
              />
            </div>

            {resultText ? (
              <Alert role="status">
                <AlertDescription>{resultText}</AlertDescription>
              </Alert>
            ) : null}
            {error ? (
              <Alert variant="destructive">
                <AlertDescription>{error}</AlertDescription>
              </Alert>
            ) : null}
          </CardContent>
        </Card>

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

function encodeSelectValue(value: string) {
  return value === "" ? EMPTY_SELECT_VALUE : value;
}

function decodeSelectValue(value: string) {
  return value === EMPTY_SELECT_VALUE ? "" : value;
}
