import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, CheckCircle2, Download, PackageCheck, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
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
          <label className="flex items-center gap-2 text-sm">
            <input
              checked={preRelease}
              className="size-4 accent-primary"
              onChange={(event) => togglePreRelease(event.currentTarget.checked)}
              type="checkbox"
            />
            <span>{t("updates.preRelease")}</span>
          </label>
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
          <div className="flex items-start gap-2 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            <AlertTriangle className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
            <span>{error}</span>
          </div>
        ) : null}

        <div className="max-h-[24rem] overflow-auto rounded-md border">
          <table className="w-full border-collapse text-sm">
            <thead className="sticky top-0 bg-card text-left">
              <tr className="border-b">
                <th className="w-12 px-3 py-2" scope="col">
                  <span className="sr-only">{t("updates.selected")}</span>
                </th>
                <th className="px-3 py-2 font-medium" scope="col">
                  {t("updates.target")}
                </th>
                <th className="px-3 py-2 font-medium" scope="col">
                  {t("updates.version")}
                </th>
                <th className="px-3 py-2 font-medium" scope="col">
                  {t("updates.status")}
                </th>
              </tr>
            </thead>
            <tbody>
              {(status?.targets ?? []).map((target) => {
                const result = resultByTarget.get(target.id);

                return (
                  <tr key={target.id} className="border-b last:border-b-0">
                    <td className="px-3 py-2 align-top">
                      <input
                        checked={selectedIds.has(target.id)}
                        className="size-4 accent-primary"
                        onChange={(event) => toggleTarget(target.id, event.currentTarget.checked)}
                        type="checkbox"
                      />
                    </td>
                    <td className="min-w-48 px-3 py-2 align-top">
                      <div className="grid gap-1">
                        <span className="font-medium">{target.name}</span>
                        <span className="text-xs text-muted-foreground">{target.remarks}</span>
                      </div>
                    </td>
                    <td className="px-3 py-2 align-top text-muted-foreground">
                      {result?.remoteVersion ?? result?.currentVersion ?? "-"}
                    </td>
                    <td className="min-w-56 px-3 py-2 align-top">
                      <UpdateResultBadge result={result} />
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>

      <DialogFooter />
    </DialogContent>
  );
}

function UpdateResultBadge({ result }: { result: UpdateCheckResult | undefined }) {
  const { t } = useI18n();

  if (!result) {
    return <span className="text-sm text-muted-foreground">{t("updates.waiting")}</span>;
  }

  const tone = statusTone(result.status);

  return (
    <div className={cn("inline-flex max-w-full items-center gap-2 rounded-md border px-2 py-1", tone)}>
      {result.status === "upToDate" || result.status === "downloaded" ? (
        <CheckCircle2 className="size-4 shrink-0" aria-hidden="true" />
      ) : result.status === "error" ? (
        <AlertTriangle className="size-4 shrink-0" aria-hidden="true" />
      ) : null}
      <span className="truncate">{result.message}</span>
    </div>
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
      return "bg-muted text-muted-foreground";
  }
}
