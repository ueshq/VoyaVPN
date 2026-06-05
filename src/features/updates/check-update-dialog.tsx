import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, CheckCircle2, Download, PackageCheck, RefreshCw } from "lucide-react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useI18n } from "@/i18n/use-i18n";
import { checkUpdates, downloadUpdates, saveUpdatePreferences, updateStatus } from "@/ipc";
import type { UpdateCheckResult, UpdateResultStatus, UpdateStatus } from "@/ipc/bindings";
import { cn } from "@/lib/utils";

type RunMode = "check" | "download";

export function CheckUpdateDialog() {
  const { t } = useI18n();
  const [error, setError] = useState<string | null>(null);
  const [preRelease, setPreRelease] = useState(false);
  const [results, setResults] = useState<UpdateCheckResult[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [working, setWorking] = useState<RunMode | null>(null);

  useEffect(() => {
    let disposed = false;

    void updateStatus()
      .then((nextStatus) => {
        if (disposed) {
          return;
        }
        setStatus(nextStatus);
        setPreRelease(nextStatus.preRelease);
        setSelectedIds(new Set(nextStatus.targets.filter((target) => target.selected).map((target) => target.id)));
      })
      .catch((error: unknown) => {
        if (!disposed) {
          setError(error instanceof Error ? error.message : String(error));
        }
      });

    return () => {
      disposed = true;
    };
  }, []);

  const selectedTargetIds = useMemo(() => [...selectedIds], [selectedIds]);
  const resultByTarget = useMemo(
    () => new Map(results.map((result) => [result.targetId, result])),
    [results],
  );

  async function persistSelection(nextPreRelease = preRelease, nextSelected = selectedTargetIds) {
    const nextStatus = await saveUpdatePreferences(nextPreRelease, nextSelected);
    setStatus(nextStatus);
    setPreRelease(nextStatus.preRelease);
    setSelectedIds(new Set(nextStatus.targets.filter((target) => target.selected).map((target) => target.id)));
  }

  async function run(mode: RunMode) {
    setWorking(mode);
    setError(null);
    try {
      await persistSelection();
      const runResult =
        mode === "check"
          ? await checkUpdates(preRelease, selectedTargetIds, true, null)
          : await downloadUpdates(preRelease, selectedTargetIds, true, null);
      setStatus({ preRelease: runResult.preRelease, targets: runResult.targets });
      setResults(runResult.results);
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      setWorking(null);
    }
  }

  function toggleTarget(id: string, checked: boolean) {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (checked) {
        next.add(id);
      } else {
        next.delete(id);
      }
      void persistSelection(preRelease, [...next]).catch((error: unknown) =>
        setError(error instanceof Error ? error.message : String(error)),
      );

      return next;
    });
  }

  function togglePreRelease(checked: boolean) {
    setPreRelease(checked);
    void persistSelection(checked, selectedTargetIds).catch((error: unknown) =>
      setError(error instanceof Error ? error.message : String(error)),
    );
  }

  return (
    <DialogContent className="max-w-3xl">
      <DialogHeader>
        <DialogTitle className="flex items-center gap-2">
          <PackageCheck className="size-4" aria-hidden="true" />
          {t("updates.title")}
        </DialogTitle>
        <DialogDescription className="sr-only">{t("updates.description")}</DialogDescription>
      </DialogHeader>

      <div className="grid gap-4">
        <div className="flex flex-wrap items-center gap-3">
          <div className="flex items-center gap-2">
            <Checkbox
              checked={preRelease}
              id="updates-pre-release"
              onCheckedChange={(checked) => togglePreRelease(checked === true)}
            />
            <Label className="cursor-pointer text-sm font-normal" htmlFor="updates-pre-release">
              {t("updates.preRelease")}
            </Label>
          </div>
          <div className="ms-auto flex items-center gap-2">
            <Button
              disabled={working !== null || selectedIds.size === 0}
              onClick={() => void run("check")}
              type="button"
              variant="outline"
            >
              <RefreshCw className={cn("size-4", working === "check" && "animate-spin")} aria-hidden="true" />
              {t("updates.check")}
            </Button>
            <Button
              disabled={working !== null || selectedIds.size === 0}
              onClick={() => void run("download")}
              type="button"
            >
              <Download className={cn("size-4", working === "download" && "animate-pulse")} aria-hidden="true" />
              {t("updates.download")}
            </Button>
          </div>
        </div>

        {error ? (
          <Alert variant="destructive">
            <AlertTriangle aria-hidden="true" />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        <ScrollArea className="h-[24rem] rounded-md border">
          <Table className="min-w-[42rem]">
            <TableHeader className="sticky top-0 z-10 bg-card">
              <TableRow className="hover:bg-transparent">
                <TableHead className="w-12 px-3" scope="col">
                  <span className="sr-only">{t("updates.selected")}</span>
                </TableHead>
                <TableHead className="px-3" scope="col">
                  {t("updates.target")}
                </TableHead>
                <TableHead className="px-3" scope="col">
                  {t("updates.version")}
                </TableHead>
                <TableHead className="px-3" scope="col">
                  {t("updates.status")}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {(status?.targets ?? []).map((target) => {
                const result = resultByTarget.get(target.id);

                return (
                  <TableRow key={target.id}>
                    <TableCell className="px-3 py-2 align-top">
                      <Checkbox
                        aria-label={`${t("updates.selected")} ${target.name}`}
                        checked={selectedIds.has(target.id)}
                        onCheckedChange={(checked) => toggleTarget(target.id, checked === true)}
                      />
                    </TableCell>
                    <TableCell className="min-w-48 whitespace-normal px-3 py-2 align-top">
                      <div className="grid gap-1">
                        <span className="font-medium">{target.name}</span>
                        <span className="text-xs text-muted-foreground">{target.remarks}</span>
                      </div>
                    </TableCell>
                    <TableCell className="px-3 py-2 align-top text-muted-foreground">
                      {result?.remoteVersion ?? result?.currentVersion ?? "-"}
                    </TableCell>
                    <TableCell className="min-w-56 px-3 py-2 align-top">
                      <UpdateResultBadge result={result} />
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </ScrollArea>
      </div>

      <DialogFooter />
    </DialogContent>
  );
}

function UpdateResultBadge({ result }: { result: UpdateCheckResult | undefined }) {
  const { t } = useI18n();

  if (!result) {
    return <Badge variant="secondary">{t("updates.waiting")}</Badge>;
  }

  const tone = statusTone(result.status);

  return (
    <Badge className={cn("max-w-full justify-start gap-2 px-2 py-1", tone)} variant="outline">
      {result.status === "upToDate" || result.status === "downloaded" ? (
        <CheckCircle2 className="shrink-0" aria-hidden="true" />
      ) : result.status === "error" ? (
        <AlertTriangle className="shrink-0" aria-hidden="true" />
      ) : null}
      <span className="truncate">{result.message}</span>
    </Badge>
  );
}

function statusTone(status: UpdateResultStatus) {
  switch (status) {
    case "downloaded":
    case "upToDate":
      return "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300";
    case "updateAvailable":
      return "border-primary/30 bg-primary/10 text-primary";
    case "error":
      return "border-destructive/40 bg-destructive/10 text-destructive";
    case "skipped":
      return "border-transparent bg-muted text-muted-foreground";
  }
}
